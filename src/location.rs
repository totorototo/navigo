use crate::{Area, Elevation};

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Location {
    pub longitude: f64,
    pub latitude: f64,
    pub altitude: f64,
}

impl Location {
    pub fn calculate_distance_to(&self, to: &Location) -> f64 {
        let earth_radius_kilometer = 6371.0;

        let origin_latitude = self.latitude.to_radians();
        let destination_latitude = to.latitude.to_radians();

        let delta_latitude = (self.latitude - to.latitude).to_radians();
        let delta_longitude = (self.longitude - to.longitude).to_radians();

        let central_angle_inner = (delta_latitude / 2.0).sin().powi(2)
            + origin_latitude.cos()
                * destination_latitude.cos()
                * (delta_longitude / 2.0).sin().powi(2);
        let central_angle = 2.0 * central_angle_inner.sqrt().asin();

        earth_radius_kilometer * central_angle
    }

    pub fn calculate_bearing_to(&self, to: &Location) -> f64 {
        let origin_latitude = self.latitude.to_radians();
        let destination_latitude = to.latitude.to_radians();
        let origin_longitude = self.longitude.to_radians();
        let destination_longitude = to.longitude.to_radians();

        let delta_longitude = (to.longitude - self.longitude).to_radians();

        let y = delta_longitude.sin() * destination_latitude.cos();
        let x = origin_latitude.cos() * destination_latitude.sin()
            - origin_latitude.sin()
                * destination_latitude.cos()
                * (destination_longitude - origin_longitude).cos();
        let teta = y.atan2(x);

        ((teta * 180.0) / std::f64::consts::PI + 360.0) % 360.0
    }

    pub fn calculate_elevation_to(&self, to: &Location) -> Elevation {
        let delta = to.altitude - self.altitude;
        match delta > 0.0 {
            true => Elevation {
                positive: delta,
                negative: 0.0,
            },
            false => Elevation {
                negative: delta.abs(),
                positive: 0.0,
            },
        }
    }

    pub fn is_in_area(&self, area: &Area) -> bool {
        self.longitude > area.min_longitude
            && self.longitude < area.max_longitude
            && self.latitude > area.min_latitude
            && self.latitude < area.max_latitude
    }

    pub fn is_in_radius(&self, location: &Location, radius: &f64) -> bool {
        let distance = self.calculate_distance_to(location);
        // println!("distance: {}", distance);
        distance - radius < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, Location};

    #[test]
    fn calculate_distance_to_location() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 0.0,
        };

        let moscow = Location {
            longitude: 37.617634,
            latitude: 55.755787,
            altitude: 0.0,
        };

        let distance = paris.calculate_distance_to(&moscow);

        assert_eq!(distance, 2486.340992526076);
    }

    #[test]
    fn calculate_bearing_to_location() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 0.0,
        };

        let moscow = Location {
            longitude: 37.617634,
            latitude: 55.755787,
            altitude: 0.0,
        };

        let bearing = paris.calculate_bearing_to(&moscow);

        assert_eq!(bearing, 58.65519236183434);
    }

    #[test]
    fn location_should_be_in_area() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 0.0,
        };

        let area = Area {
            max_latitude: 56.755787,
            min_latitude: 54.755787,
            min_longitude: 36.617634,
            max_longitude: 38.617634,
        };

        let is_in = paris.is_in_area(&area);

        assert_eq!(is_in, false);
    }

    #[test]
    fn location_should_be_in_radius() {
        let center = Location {
            longitude: 6.23828,
            latitude: 45.50127,
            altitude: 0.0,
        };
        let location = Location {
            longitude: 5.77367,
            latitude: 45.07122,
            altitude: 0.0,
        };

        let radius = 70000.00;
        let is_in = location.is_in_radius(&center, &radius);

        assert_eq!(is_in, true);
    }

    #[test]
    fn calculate_elevation_to() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 0.0,
        };

        let moscow = Location {
            longitude: 37.617634,
            latitude: 55.755787,
            altitude: 200.0,
        };

        let elevation = paris.calculate_elevation_to(&moscow);

        assert_eq!(200.0, elevation.positive);
        assert_eq!(0.0, elevation.negative);
    }
}
