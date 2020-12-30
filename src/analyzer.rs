use crate::{Elevation, Location, Trace};

// instead of receiving the values `Trace`,
// Trace borrows (pre-existing) instances of Trace
#[derive(Debug)]
pub struct Analyzer<'a> {
    pub trace: &'a Trace,
    statistics: Vec<Stats>,
}

#[derive(Debug)]
pub struct Stats {
    distance: f64,
    elevation: Elevation,
}

impl<'a> Analyzer<'a> {
    pub fn new(trace: &'a Trace) -> Self {
        // use std::time::Instant;
        // let now = Instant::now();

        let statistics = Analyzer::compute_analytics(trace);
        // let elapsed = now.elapsed();
        // println!("Elapsed: {:?}", elapsed);

        Analyzer { trace, statistics }
    }

    fn compute_analytics(trace: &Trace) -> Vec<Stats> {
        let stat = Stats {
            distance: 0.0,
            elevation: Elevation {
                positive: 0.0,
                negative: 0.0,
            },
        };
        let initial_value = vec![stat];

        let mut cumulative_distance = 0.0;
        let mut cumulative_elevation_gain = 0.0;
        let mut cumulative_elevation_loss = 0.0;

        trace
            .locations
            .iter()
            .enumerate()
            .fold(initial_value, |mut acc, (index, location)| {
                let next = trace.locations.iter().nth(index + 1);
                match next {
                    Some(next_location) => {
                        let distance = location.calculate_distance_to(next_location);
                        cumulative_distance += distance;

                        let elevation = location.calculate_elevation_to(next_location);
                        cumulative_elevation_gain += elevation.positive;
                        cumulative_elevation_loss += elevation.negative;

                        let stat = Stats {
                            distance: cumulative_distance,
                            elevation: Elevation {
                                positive: cumulative_elevation_gain,
                                negative: cumulative_elevation_loss,
                            },
                        };

                        acc.push(stat);
                        acc
                    }
                    None => acc,
                }
            })
    }

    pub fn compute_closest_location(&self, current_location: &Location) -> Result<&Location, &str> {
        if self.trace.locations.len() == 0 {
            return Err("empty trace");
        }

        let first_location = self.trace.locations.first();
        let initial_value = match first_location {
            Some(location) => location,
            None => return Err("could not retrieve first trace location"),
        };

        let location = self
            .trace
            .locations
            .iter()
            .fold(initial_value, |acc, location| {
                // TODO: could be improved (computed twice)
                let ref_distance = current_location.calculate_distance_to(acc);
                let new_distance = current_location.calculate_distance_to(location);
                match new_distance < ref_distance {
                    true => location,
                    false => acc,
                }
            });
        Ok(location)
    }

    pub fn find_location_index(&self, current_location: &Location) -> Result<usize, &str> {
        let position = self.trace.locations.iter().position(|location| {
            location.longitude == current_location.longitude
                && location.latitude == current_location.latitude
                && location.altitude == current_location.altitude
        });
        match position {
            Some(position) => Ok(position),
            None => Err("not found"),
        }
    }

    pub fn find_location_index_at(&self, distance: f64) -> Result<usize, &str> {
        if distance < 0.0 {
            return Err("negative mark");
        }

        if self.statistics.len() == 0 {
            return Err("no statistics computed yet");
        }

        if self.trace.locations.len() == 0 {
            return Err("could not compute statistics for empty trace");
        }

        let first_stats = self.statistics.first();
        let initial_distance = match first_stats {
            Some(stats) => stats.distance,
            None => return Err("could not retrieve first stats item"),
        };

        let mut delta = (distance - initial_distance).abs();

        let index = self
            .statistics
            .iter()
            .enumerate()
            .fold(0, |acc, (index, statistic)| {
                let diff = (distance - statistic.distance).abs();

                match diff < delta {
                    true => {
                        delta = diff;
                        index
                    }
                    false => acc,
                }
            });

        if index > self.trace.locations.len() {
            return Err("out of bounds");
        }
        Ok(index)
    }

    pub fn get_location_at(&self, distance: f64) -> Result<&Location, &str> {
        self.trace
            .locations
            .iter()
            .nth(self.find_location_index_at(distance)?)
            .ok_or_else(|| "location not found")
    }

    pub fn get_trace_section(&self, start: f64, end: f64) -> Result<Vec<Location>, &str> {
        let last_stats = self.statistics.last();

        let stats = match last_stats {
            Some(stats) => stats,
            None => return Err("no computed stats"),
        };

        if stats.distance < end {
            return Err("out of bounds");
        };

        if start < 0.0 {
            return Err("negative values");
        };

        let start_index = self.find_location_index_at(start)?;
        let end_index = self.find_location_index_at(end)?;
        self.trace.get_section(start_index, end_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{helper, Location};

    #[test]
    fn should_find_location_index() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);

        let index = analyzer.find_location_index_at(0.2);
        assert_eq!(index.is_ok(), true);
        assert_eq!(index.ok(), Some(4));
    }

    #[test]
    fn should_return_error_while_getting_location_for_wrong_mark() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);

        let location = analyzer.get_location_at(-100.2);

        assert_eq!(location.is_ok(), false);
        assert_eq!(location.err(), Some("negative mark"));
    }

    #[test]
    fn should_get_location_for_200_mark() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let matched_location = Location {
            longitude: 0.328684,
            latitude: 42.828782,
            altitude: 793.0,
        };

        let analyzer = Analyzer::new(&trace);

        let location = analyzer.get_location_at(0.2);

        assert_eq!(location.is_ok(), true);
        assert_eq!(location.ok(), Some(&matched_location));
    }

    #[test]
    fn should_return_error_while_getting_closest_location() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 800.00,
        };

        let locations = vec![];
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);

        let location = analyzer.compute_closest_location(&paris);

        assert_eq!(location.is_ok(), false);
        assert_eq!(location.err(), Some("empty trace"));
    }

    #[test]
    fn compute_closest_location() {
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

        let analyzer = Analyzer::new(&trace);

        let current_location = Location {
            longitude: 1.350987,
            latitude: 49.856667,
            altitude: 800.00,
        };

        let closest_location = analyzer.compute_closest_location(&current_location);

        assert_eq!(closest_location.is_ok(), true);
        assert_eq!(closest_location.ok(), Some(&paris));
    }

    #[test]
    fn find_location_index() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);

        let current_location = Location {
            longitude: 5.77501,
            latitude: 45.07069,
            altitude: 281.516,
        };

        let index = analyzer.find_location_index(&current_location);

        assert_eq!(index.is_ok(), false);
        assert_eq!(index.err(), Some("not found"));
    }

    #[test]
    fn should_not_get_any_location_index() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);

        let current_location = Location {
            longitude: 5.7750,
            latitude: 45.07069,
            altitude: 281.516,
        };

        let index = analyzer.find_location_index(&current_location);
        assert_eq!(index.is_ok(), false);
        assert_eq!(index.err(), Some("not found"));
    }

    #[test]
    fn should_get_a_location_section() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let section = vec![
            Location {
                longitude: 0.32802,
                latitude: 42.829719999999995,
                altitude: 792.0,
            },
            Location {
                longitude: 0.32839999999999997,
                latitude: 42.8296,
                altitude: 792.0,
            },
            Location {
                longitude: 0.32882,
                latitude: 42.829181,
                altitude: 792.0,
            },
        ];

        let analyzer = Analyzer::new(&trace);
        let computed_section = analyzer.get_trace_section(0.0, 0.2);

        assert_eq!(computed_section.is_ok(), true);
        assert_eq!(Some(section), computed_section.ok())
    }

    #[test]
    fn should_return_error_while_getting_trace_section() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);
        let section = analyzer.get_trace_section(0.0, 10000.2);

        assert_eq!(section.is_ok(), false);
        assert_eq!(section.err(), Some("out of bounds"));
    }

    #[test]
    fn should_return_error_while_getting_trace_section_negative_value() {
        let locations = helper::get_locations();
        let trace = Trace { locations };

        let analyzer = Analyzer::new(&trace);
        let section = analyzer.get_trace_section(-20.0, 0.2);

        assert_eq!(section.is_ok(), false);
        assert_eq!(section.err(), Some("negative values"));
    }
}
