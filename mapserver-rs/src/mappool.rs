use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{bounded, select, Sender};
use libc;
use threadpool::ThreadPool;

use mapserver_sys::{
    mapObj, msCleanup, msDebugCleanup, msDrawMap, msFreeImage, msFreeMap, msGDALCleanup,
    msIO_Cleanup, msLoadMapFromString, msMapSetExtent, msOGRCleanup,
    msProjectionContextPoolCleanup, msSaveImageBuffer, msSetPROJ_DATA,
};

use super::Extent;

const MAP_IDLE_TIMEOUT_SECONDS: u64 = 60 * 60;

///
/// The Map struct manages the Mapserver mapObj lifecycle
///
pub struct Map {
    map_obj: *mut mapObj,
}

impl Map {
    pub fn from(mapfile_contents: String) -> Self {
        // Convert mapfile contents to *char
        let mapfile_cstr = CString::new(mapfile_contents).unwrap();
        let buffer = mapfile_cstr.as_ptr() as *mut c_char;

        let map_obj = unsafe { msLoadMapFromString(buffer, std::ptr::null_mut() as *mut c_char) };
        if map_obj.is_null() {
            panic!("Unable to load mapfile");
        }
        Map { map_obj }
    }

    pub fn draw(&self, ext: Extent) -> Vec<u8> {
        let mut size = 0;

        let result_ptr = unsafe {
            msMapSetExtent(self.map_obj, ext.0, ext.1, ext.2, ext.3);
            // Draw map
            let img = msDrawMap(self.map_obj, 0);
            if img.is_null() {
                panic!("Unable to render map");
            }

            // Save the image and convert to a u8 slice
            let result_ptr = msSaveImageBuffer(img, &mut size, (*img).format);
            msFreeImage(img);
            result_ptr
        };

        let img_bytes = unsafe { std::slice::from_raw_parts(result_ptr, size as usize).to_owned() };

        unsafe {
            // Free the image and the temporary buffer
            libc::free(result_ptr as *mut libc::c_void);
        };

        img_bytes
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        unsafe {
            // We cannot do a full msCleanup() or msGDALCleanup() here
            msFreeMap(self.map_obj);
            msDebugCleanup();
        }
    }
}

///
/// MapRenderChannel wraps two channels, forming a bidirectional channel
/// to receive extents and send images
///
#[derive(Debug, Clone)]
pub struct MapRenderChannel {
    extent_sender: crossbeam_channel::Sender<Extent>,
    img_receiver: crossbeam_channel::Receiver<Vec<u8>>,
}

impl MapRenderChannel {
    pub fn render(&self, ext: Extent) -> Vec<u8> {
        match self.extent_sender.send(ext) {
            Ok(_) => self.img_receiver.recv().unwrap(),
            Err(_) => todo!("MapRenderThread is not alive, this should never happen"),
        }
    }
}

///
/// MapPool manages a threadpool, one thread per logical mapfile
/// and provides a locked lookup-table to ensure singleton access
/// to Map / Dataset instantiation. The render loop is single-threaded
/// (though underlying IO can be multithreaded)
/// and uses channels to communicate back to the main task.
///
#[derive(Debug)]
pub struct MapPool {
    lookup: Arc<Mutex<HashMap<String, MapRenderChannel>>>,
    threads: ThreadPool,
    exit_sender: Sender<String>,
}

impl MapPool {
    pub fn acquire_or_create(&mut self, mapfile_str: String) -> MapRenderChannel {
        let mut lookup = self.lookup.lock().unwrap();

        let result = lookup.entry(mapfile_str.clone()).or_insert_with(|| {
            // Pair of zero-bounded "rendevous" channels mimic request-response
            let (extent_sender, extent_receiver) = bounded(0);
            let (img_sender, img_receiver) = bounded(0);

            let threadpool = self.threads.clone();
            let mapfile_str2 = mapfile_str.clone();
            let exit = self.exit_sender.clone();

            threadpool.execute(move || {
                let map = Map::from(mapfile_str2);
                loop {
                    select! {
                      recv(extent_receiver) -> extent => {
                          if let Ok(extent) = extent {
                              let img = map.draw(extent);
                              img_sender.send(img).unwrap();
                          } else {
                              break
                          }
                      },
                      default(Duration::from_secs(MAP_IDLE_TIMEOUT_SECONDS)) => break,
                    }
                }
                exit.send(mapfile_str).unwrap();
            });

            MapRenderChannel {
                extent_sender,
                img_receiver,
            }
        });
        result.clone()
    }

    pub fn create(size: usize) -> Self {
        let lookup = Arc::new(Mutex::new(HashMap::new()));
        let threads = ThreadPool::with_name("MapserverThreadPool".into(), size + 1);
        let (exit_sender, exit_receiver): (
            crossbeam_channel::Sender<String>,
            crossbeam_channel::Receiver<String>,
        ) = bounded(0);

        let map_lookup = lookup.clone();

        // Spawn a "Garbage Collection" thread
        threads.execute(move || loop {
            while let Ok(exited_mapfile) = exit_receiver.recv() {
                let mut lk = map_lookup.lock().unwrap();
                lk.remove(&exited_mapfile).unwrap();
                if lk.len() == 0 {
                    // All maps are dropped, only now is it safe to cleanup
                    unsafe {
                        // We cannot do a full msCleanup() here either :-/
                        // What *can* we safely cleanup without fully unloading the shared library?
                        msGDALCleanup();
                        msOGRCleanup();
                        msIO_Cleanup();
                        msSetPROJ_DATA(std::ptr::null(), std::ptr::null());
                        msProjectionContextPoolCleanup();
                    }
                }
            }
        });

        MapPool {
            lookup,
            threads,
            exit_sender,
        }
    }
}

impl Drop for MapPool {
    fn drop(&mut self) {
        unsafe {
            msCleanup();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mappool() {
        let mapfile_str = "MAP END".to_string();
        let mut map_pool = MapPool::create(20);
        let mapthread = map_pool.acquire_or_create(mapfile_str);

        let extent = Extent(
            -11711375.725741565,
            4940736.634297222,
            -11711222.851684995,
            4940889.508353792,
        );
        let img = mapthread.render(extent);

        // The resulting png-encoded image is likely > 10kb
        assert!(img.len() >= 10_000);
    }
}
