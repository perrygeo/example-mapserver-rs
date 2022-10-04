use std::sync::Arc;

use mapserver_rs::coordinates::Tile;
use mapserver_rs::mappool::MapPool;
use mapserver_rs::Extent;

use axum::extract::Path;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::Extension;
use axum::{routing::get, Router};
use tokio::sync::Mutex;

pub fn make_mapfile_str(timestamp: i64) -> String {
    format!(
        "MAP
          NAME 'default'
          STATUS ON
          PROJECTION
            'init=epsg:3857'
          END
          EXTENT -11711375.725741563 4941042.382410363 -11711222.851684993 4941195.256466932
          UNITS METERS
          DEBUG 5
          CONFIG 'CPL_DEBUG' 'ON'
          CONFIG 'CPL_TIMESTAMP' 'ON'
          CONFIG 'CPL_LOG' '/dev/stderr'
          CONFIG 'CPL_LOG_ERRORS' 'ON'
          CONFIG 'MS_ERRORFILE' '/dev/stderr'
          CONFIG 'GDAL_DISABLE_READDIR_ON_OPEN' 'TRUE'
          CONFIG 'GDAL_FORCE_CACHING' 'NO'
          CONFIG 'GDAL_CACHEMAX' '10%'
          CONFIG 'VSI_CACHE' 'FALSE'
          CONFIG 'VSI_CACHE_SIZE' '0'  # bytes
          CONFIG 'CPL_VSIL_CURL_CACHE_SIZE' '0'  # bytes
          SIZE 256 256
          IMAGECOLOR 255 255 255
          IMAGETYPE 'png'
          SHAPEPATH '/tmp'
          LAYER
            NAME 'default'
            TYPE RASTER
            STATUS ON
            DEBUG 5
            PROJECTION
              AUTO
            END
            DATA '/home/mperry/work/tiledb/naip/naip-combined'
            # DATA 's3://perrygeo-tiledb/arrays/naip-2017'
            CONNECTIONOPTIONS
              'TILEDB_CONFIG'	'/home/mperry/work/tiledb/tiledb.aws.config'
              'TILEDB_TIMESTAMP' '{}'
            END
            PROCESSING 'CLOSE_CONNECTION=DEFER'
            PROCESSING 'BANDS=1,2,3,4'
            PROCESSING 'SCALE_4=0,1'  # Hack to ignore band 4`
          END
        END",
        timestamp
    )
}

#[derive(Debug)]
struct State {
    maplock: Mutex<MapPool>,
}

#[tokio::main]
async fn main() {
    // Set up shared state
    let map_pool = MapPool::create(24);
    let shared_state = Arc::new(State {
        maplock: Mutex::new(map_pool),
    });

    // Routes
    let app = Router::new()
        .route("/", get(index))
        .route("/map/:timestamp/:z/:x/:y", get(render_map))
        .layer(Extension(shared_state));

    // Spawn the web handler
    tokio::spawn(async move {
        println!("Listening on 0.0.0.0:3000");
        axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    // And wait for an interupt signal
    tokio::signal::ctrl_c().await.unwrap();
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn render_map(
    Path((timestamp, z, x, y)): Path<(i64, u32, u32, u32)>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    // Create mapfile
    let tile = Tile::from_zxy(z, x, y);
    let extent = Extent::from(tile.bbox_mercator());
    let mapfile_str = make_mapfile_str(timestamp);

    // Get a renderer from the map pool
    let renderer = {
        let mut map_pool = state.maplock.lock().await;
        map_pool.acquire_or_create(mapfile_str)
    };

    // Yes, we can render concurrently on multiple threads!
    // GDAL may lock things internally though, negating much of the benefit
    let image_bytes = renderer.render(extent);

    ([(header::CONTENT_TYPE, "image/png")], image_bytes)
}
