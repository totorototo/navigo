use crate::data::COORDINATES;
use crate::Location;

#[allow(dead_code)]
pub fn get_locations() -> Vec<Location> {
    COORDINATES
        .iter()
        .map(|x| Location {
            longitude: x[0],
            latitude: x[1],
            altitude: x[2],
        })
        .collect()
}
