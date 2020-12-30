use crate::data::COORDINATES;
use crate::Location;

#[allow(dead_code)]
pub fn get_locations() -> Vec<Location> {
    COORDINATES
        .iter()
        .fold(Vec::new(), |mut acc: Vec<Location>, x| {
            acc.push(Location {
                longitude: x[0],
                altitude: x[2],
                latitude: x[1],
            });
            acc
        })
}
