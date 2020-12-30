use crate::{Area, Elevation, Location};

#[derive(Debug)]
pub struct Trace {
    pub locations: Vec<Location>,
}

impl Trace {
    pub fn length(&self) -> f64 {
        self.locations
            .iter()
            .enumerate()
            .fold(0.0, |acc, (index, location)| {
                let next = self.locations.iter().nth(index + 1);
                match next {
                    Some(next_location) => {
                        let distance = location.calculate_distance_to(next_location);
                        acc + distance
                    }
                    None => acc,
                }
            })
    }

    pub fn elevation(&self) -> Elevation {
        self.locations.iter().enumerate().fold(
            Elevation {
                positive: 0.00,
                negative: 0.00,
            },
            |mut acc, (index, location)| {
                let next = self.locations.iter().nth(index + 1);
                match next {
                    Some(next_location) => {
                        let delta = next_location.altitude - location.altitude;
                        match delta > 0.0 {
                            true => {
                                acc.positive += delta;
                                acc
                            }
                            false => {
                                acc.negative += delta.abs();
                                acc
                            }
                        }
                    }
                    None => acc,
                }
            },
        )
    }

    pub fn area(&self) -> Result<Area, &str> {
        if self.locations.len() == 0 {
            return Err("could not compute area");
        }

        let first_location = match self.locations.iter().nth(0) {
            Some(location) => location,
            None => return Err("unable to retrieve first location"),
        };

        let initial_state = Area {
            min_longitude: first_location.longitude,
            max_longitude: first_location.longitude,
            min_latitude: first_location.latitude,
            max_latitude: first_location.latitude,
        };

        let area = self
            .locations
            .iter()
            .fold(initial_state, |acc, location| Area {
                min_latitude: acc.min_latitude.min(location.latitude),
                max_latitude: acc.max_latitude.max(location.latitude),
                min_longitude: acc.min_longitude.min(location.longitude),
                max_longitude: acc.max_longitude.max(location.longitude),
            });

        Ok(area)
    }

    pub fn get_section(&self, start_index: usize, end_index: usize) -> Result<Vec<Location>, &str> {
        if self.locations.len() == 0 {
            return Err("could not compute section");
        }

        let section =
            self.locations
                .iter()
                .enumerate()
                .fold(Vec::new(), |mut acc, (index, item)| {
                    if index > start_index && index < end_index {
                        acc.push(*item);
                        acc
                    } else {
                        acc
                    }
                });

        Ok(section)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::helper;

    #[test]
    fn compute_trace_elevation() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let elevation = trace.elevation();

        assert_eq!(elevation.negative, 17666.0);
        assert_eq!(elevation.positive, 17665.0);
    }

    #[test]
    fn compute_trace_distance() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let length = trace.length();

        assert_eq!(length, 219.39059361170354);
    }

    #[test]
    fn compute_area_for_trace() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 800.00,
        };

        let moscow = Location {
            longitude: 37.617634,
            latitude: 55.755787,
            altitude: 200.00,
        };

        let locations = vec![paris, moscow];
        let trace = Trace { locations };

        let computed_area = trace.area();
        assert_eq!(computed_area.is_ok(), true);

        let area = Area {
            min_longitude: 2.350987,
            max_longitude: 37.617634,
            min_latitude: 48.856667,
            max_latitude: 55.755787,
        };

        assert_eq!(computed_area.ok(), Some(area));
    }

    #[test]
    fn should_return_error_while_computing_area_for_empty_trace() {
        let locations = vec![];
        let trace = Trace { locations };

        let computed_area = trace.area();

        assert_eq!(computed_area.is_ok(), false);
        assert_eq!(computed_area.err(), Some("could not compute area"));
    }

    #[test]
    fn should_return_error_while_getting_trace_section_for_empty_trace() {
        let locations = vec![];
        let trace = Trace { locations };

        let computed_section = trace.get_section(0, 10);

        assert_eq!(computed_section.is_ok(), false);
        assert_eq!(computed_section.err(), Some("could not compute section"));
    }

    #[test]
    fn should_retrieve_section() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let section = vec![
            Location {
                longitude: 0.32882,
                latitude: 42.829181,
                altitude: 792.0,
            },
            Location {
                longitude: 0.328684,
                latitude: 42.828782,
                altitude: 793.0,
            },
        ];

        let computed_section = trace.get_section(2, 5);

        assert_eq!(computed_section.is_ok(), true);
        assert_eq!(Some(section), computed_section.ok())
    }
}
