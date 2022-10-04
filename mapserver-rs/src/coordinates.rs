//! Convert between coordinates for geospatial points, bounds and tiles
//!
//! ## Tiles
//!
//! ```
//! use mapserver_rs::coordinates::Tile;
//!
//! // ~Denver, Colorado, USA
//! // see https://a.tile.openstreetmap.org/7/26/48.png
//! let t = Tile::from_coords(-105., 40., 7);
//! assert_eq!(t.zoom, 7);
//! assert_eq!(t.x, 26);
//! assert_eq!(t.y, 48);
//!
//! // Get children from zoom 9 up
//! // Zoom 9 is 16 tiles
//! // Zoom 8 is 4 tiles
//! // Zoom 7 (parent) is 1 tile
//! let children = t.children(9);
//! assert_eq!(children.len(), 21);
//! assert_eq!(children[0].zoom, 9);
//! ```
//!

use std::f64::consts::{E, PI};

const EARTH_RADIUS: f64 = 6378137.0;
const EARTH_CIRCUMFERENCE: f64 = 2. * PI * EARTH_RADIUS;

/// A Web Mercator ZXY tile
#[derive(Clone, Debug)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub zoom: u32,
}

impl Tile {
    pub fn from_zxy(z: u32, x: u32, y: u32) -> Self {
        Tile { x, y, zoom: z }
    }

    /// Convert a longitude and latitude to the bounding Tile
    /// at a given zoom level
    pub fn from_coords(lon: f64, lat: f64, zoom: u32) -> Self {
        let latsin = lat.to_radians().sin();
        let z2: f64 = (2.0f64).powf(zoom as f64);

        // Normalize
        let x = 0.5 + lon / 360.;
        let y = 0.5 - 0.25 * ((1. + latsin) / (1. - latsin)).log(E) / std::f64::consts::PI;

        // X Tile
        let xtile = if x <= 0. {
            0
        } else if x >= 1. {
            z2 as u32 - 1
        } else {
            (x * z2).floor() as u32
        };

        // Y Tile
        let ytile = if y <= 0. {
            0
        } else if y >= 1. {
            z2 as u32 - 1
        } else {
            (y * z2).floor() as u32
        };

        Tile {
            x: xtile,
            y: ytile,
            zoom,
        }
    }

    /// Convert zxy to bounding coordinates of tile in epsg:3857
    pub fn bbox_mercator(&self) -> (f64, f64, f64, f64) {
        let tile_size = EARTH_CIRCUMFERENCE / (2.0f64).powf(self.zoom as f64);

        let llx = self.x as f64 * tile_size - (EARTH_CIRCUMFERENCE / 2.);
        let urx = llx + tile_size;
        let ury = (EARTH_CIRCUMFERENCE / 2.) - self.y as f64 * tile_size;
        let lly = ury - tile_size;

        (llx, lly, urx, ury)
    }

    pub fn url_zyx(&self, template: String) -> String {
        let mut url = template;
        url = url.replace("{x}", self.x.to_string().as_ref());
        url = url.replace("{y}", self.y.to_string().as_ref());
        url = url.replace("{z}", self.zoom.to_string().as_ref());
        url
    }

    pub fn url_wms(&self, template: String) -> String {
        let bbox = self.bbox_mercator();
        let bbox = format!("{},{},{},{}", bbox.0, bbox.1, bbox.2, bbox.3);

        let mut url = template;
        url = url.replace("{bbox}", &bbox);
        url = url.replace("{srs}", "EPSG:3857");
        url
    }

    /// Get all children of the parent `Tile`.
    /// In reverse order, graudally zooms out
    /// Final element includes the parent tile
    pub fn children(&self, target_zoom: u32) -> Vec<Self> {
        let metatile = Tile {
            x: self.x,
            y: self.y,
            zoom: self.zoom,
        };
        let mut tiles = vec![metatile];
        // Iterate over tiles repeatedly, breaking each tile into four for the next zoom level
        // TODO might be more efficient with recursion
        for z in self.zoom..target_zoom {
            for t in tiles.clone().iter() {
                if t.zoom == z {
                    tiles.push(Tile {
                        x: t.x * 2,
                        y: t.y * 2,
                        zoom: z + 1,
                    });
                    tiles.push(Tile {
                        x: t.x * 2 + 1,
                        y: t.y * 2,
                        zoom: z + 1,
                    });
                    tiles.push(Tile {
                        x: t.x * 2 + 1,
                        y: t.y * 2 + 1,
                        zoom: z + 1,
                    });
                    tiles.push(Tile {
                        x: t.x * 2,
                        y: t.y * 2 + 1,
                        zoom: z + 1,
                    });
                }
            }
        }

        tiles.reverse();
        tiles
    }
}

mod test {
    #[test]
    fn test_tile() {
        // Front range CO, https://a.tile.openstreetmap.org/7/26/48.png
        let t = super::Tile::from_coords(-105., 40., 7);
        assert_eq!(t.zoom, 7);
        assert_eq!(t.x, 26);
        assert_eq!(t.y, 48);
    }
}
