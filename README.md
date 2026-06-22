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

- **calculate distance to another location** (km):

```rust
let distance: f64 = paris.calculate_distance_to(&moscow);
```

- **calculate bearing to another location** (degrees):

```rust
let bearing: f64 = paris.calculate_bearing_to(&moscow);
```

- **calculate elevation change to another location**:

```rust
let elevation: Elevation = paris.calculate_elevation_to(&moscow);
// elevation.positive — gain in meters
// elevation.negative — loss in meters
```

- **check if location is within a bounding box**:

```rust
let area = Area {
    min_latitude: 54.755787,
    max_latitude: 56.755787,
    min_longitude: 36.617634,
    max_longitude: 38.617634,
};
let is_in: bool = paris.is_in_area(&area);
```

- **check if location is within a radius** (meters):

```rust
let is_in: bool = location.is_in_radius(&center, &70000.0);
```

---

## trace

A trace is a sequence of GPS locations.

- **build a trace**:

```rust
let locations = vec![paris, moscow];
let trace = build_trace(&locations);
// or directly:
let trace = Trace { locations: &locations };
```

- **total length** (km):

```rust
let length: f64 = trace.length();
```

- **cumulative elevation**:

```rust
let elevation: Elevation = trace.elevation();
```

- **bounding box**:

```rust
let area: Result<Area, &str> = trace.area();
```

- **sub-section by index range** (inclusive):

```rust
let section: Result<Vec<Location>, &str> = trace.get_section(start_index, end_index);
```

---

## analyzer

The `Analyzer` computes statistics over a trace and provides spatial queries.

```rust
let analyzer = Analyzer::new(&trace);
```

- **location at a distance mark** (km):

```rust
let location: Result<&Location, &str> = analyzer.get_location_at(100.2);
```

- **elevation at a distance mark** (km):

```rust
let elevation: Result<&Elevation, &str> = analyzer.get_elevation_at(100.2);
```

- **index of the location closest to a distance mark**:

```rust
let index: Result<usize, &str> = analyzer.find_location_index_at(100.2);
```

- **index of a specific location**:

```rust
let index: Result<usize, &str> = analyzer.find_location_index(&location);
```

- **closest location in the trace to a given point**:

```rust
let closest: Result<&Location, &str> = analyzer.compute_closest_location(&current_location);
```

- **sub-section between two distance marks** (km):

```rust
let section: Result<Vec<Location>, &str> = analyzer.get_trace_section(0.0, 10.5);
```

---

## license

[MIT](LICENSE)
