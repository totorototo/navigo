use crate::Location;

const ROUTE_CSV: &str = include_str!("../tests/fixtures/route.csv");

#[allow(dead_code)]
pub fn get_locations() -> Vec<Location> {
    ROUTE_CSV
        .lines()
        .map(|line| {
            let mut parts = line.split(',');
            let longitude = parts.next().unwrap().parse().unwrap();
            let latitude = parts.next().unwrap().parse().unwrap();
            let altitude = parts.next().unwrap().parse().unwrap();
            Location {
                longitude,
                latitude,
                altitude,
            }
        })
        .collect()
}
