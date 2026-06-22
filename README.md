# navigo

simply manipulate GPS/geospatial data — in rust

# api / usage

## location

A location is a GPS coordinate defined by a longitude, a latitude, and an altitude.

```rust
let location = Location {
    longitude: 2.350987,
    latitude: 48.856667,
    altitude: 890.0,
};
```

- **distance to another location** (km):

```rust
let distance: f64 = paris.calculate_distance_to(&moscow);
```

- **bearing to another location** (degrees):

```rust
let bearing: f64 = paris.calculate_bearing_to(&moscow);
```

- **elevation change to another location**:

```rust
let elevation: Elevation = paris.calculate_elevation_to(&moscow);
// elevation.positive — gain in meters
// elevation.negative — loss in meters
```

- **check if inside a bounding box**:

```rust
let area = Area { min_latitude: 54.7, max_latitude: 56.7,
                  min_longitude: 36.6, max_longitude: 38.6 };
let is_in: bool = location.is_in_area(&area);
```

- **check if inside a radius** (km):

```rust
let is_in: bool = location.is_in_radius(&center, &70.0);
```

---

## trace

`Trace::new` ingests raw GPS locations and precomputes everything in one shot:
- Douglas-Peucker simplification (for traces > 1 000 points, ε = 15 m)
- Cumulative distances
- Denoised cumulative elevation gain / loss (median smoothing + hysteresis)
- Smoothed slope at each point
- Peaks and valleys (AMPD algorithm with prominence filter)
- Qualifying climb segments (Garmin-style thresholds)

```rust
let trace = Trace::new(&locations);
// or via the convenience wrapper:
let trace = build_trace(&locations);
```

### precomputed fields

```rust
trace.locations                   // Vec<Location>  — simplified working set
trace.cumulative_distances        // Vec<f64>        — km from start, [0] == 0.0
trace.cumulative_elevation_gains  // Vec<f64>        — denoised gain in meters
trace.cumulative_elevation_losses // Vec<f64>        — denoised loss in meters
trace.slopes                      // Vec<f64>        — % grade at each point
trace.peaks                       // Vec<usize>      — indices of detected peaks
trace.valleys                     // Vec<usize>      — indices of detected valleys
trace.climbs                      // Vec<ClimbStats> — qualifying climb segments
trace.total_distance              // f64             — total distance in km
trace.total_elevation_gain        // f64             — total denoised gain in meters
trace.total_elevation_loss        // f64             — total denoised loss in meters
```

### methods

- **total length** (km):

```rust
let length: f64 = trace.length(); // alias for total_distance
```

- **location at a cumulative distance** (km):

```rust
let loc: Option<&Location> = trace.point_at_distance(42.0);
```

- **index at a cumulative distance** (binary search):

```rust
let idx: usize = trace.index_at_distance(42.0);
```

- **slice between two distance marks** (km, both ends inclusive):

```rust
let section: Option<&[Location]> = trace.slice_between_distances(10.0, 50.0);
```

- **closest location to a point** (early-stop heuristic for loop courses):

```rust
let (loc, idx, dist_km) = trace.find_closest_point(&target).unwrap();
// start search from a known index (e.g. to handle loop courses):
let result = trace.find_closest_point_from(&target, start_from);
```

- **bounding box**:

```rust
let area: Result<Area, &str> = trace.area();
```

- **sub-section by index range** (inclusive):

```rust
let section: Result<Vec<Location>, &str> = trace.get_section(start_index, end_index);
```

### climb stats

```rust
pub struct ClimbStats {
    pub start_index:    usize, // valley index in trace.locations
    pub end_index:      usize, // summit index in trace.locations
    pub start_dist_km:  f64,
    pub climb_dist_km:  f64,
    pub elevation_gain: f64,   // meters
    pub summit_elev:    f64,   // meters
    pub avg_gradient:   f64,   // %
}
```

Climbs are qualified using Garmin Climb Pro thresholds:
`distance ≥ 500 m`, `average gradient ≥ 3 %`, `distance × gradient > 3 500 m·%`

---

## license

[MIT](LICENSE)
