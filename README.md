# navigo

simply manipulate geojson map data - in rust


# apis / usage

## location:

a location is just a gps position/coordinate defined by a longitude, a latitude and an altitude.

- define a new location:

```
 let lcoation = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 890.0,
 };
```

- calculate distance to location:

```

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
        
 let distance:f64 = paris.calculate_distance_to(&moscow);
```


- calculate bearing to location:

```
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

 let bearing:f64 = paris.calculate_bearing_to(&moscow);

```

- calculate elevation to location:

```
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

  let elevation:Elevation = paris.calculate_elevation_to(&moscow);

```

- check if location is in squared area:

```
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

 let is_in:bool = paris.is_in_area(&area);

```

- check if location is in a radius:

```
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
 let is_in:bool = location.is_in_radius(&center, &radius);
```

## trace: 

A trace is composed by following gps locations.

- create / define a trace:

```
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
```

- get trace length: 

```
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
 
  let length:f64 = trace.length();
```

- get trace elevation: 

```
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
 
 let elevation:Elevation = trace.elevation();
```

- get trace squared area: 

```
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

 let computed_area:Result<Area, &str> = trace.area();
```

## analyzer

An analyser is just a helper tool to manipulate trace locations.

- create a new analyzer: 

```
  let locations = ...;
  let trace = Trace { locations };
  
  let analyzer = Analyzer::new(&trace);
```

- get position at a given mark: 

```
  let locations = ...;
  let trace = Trace { locations };
  
  let analyzer = Analyzer::new(&trace);
  
  let location:Result<&Location, &str> = analyzer.get_location_at(100.2);
```

- get position index:

```
  let locations = ...;
  let trace = Trace { locations };
  
  let analyzer = Analyzer::new(&trace);  
  let location:Result<usize, &str> = analyzer.find_location_index_at(100.2);
```

- compute the closest location from trace:

```
  let locations = ...;
  let trace = Trace { locations };
  
  let analyzer = Analyzer::new(&trace);
  
  let current_location = Location {
            longitude: 1.350987,
            latitude: 49.856667,
            altitude: 800.00,
  };
  
  let closest_location:Result<&Location, &str> = analyzer.compute_closest_location(&current_location);
```


- get trace section: 

```
  let locations = ...;
  let trace = Trace { locations };
  
  let analyzer = Analyzer::new(&trace);  
  let sub_section:Result<Vec<Location>, &str> = analyzer.get_trace_section(0.0, 0.2);
```

## license

[MIT](LICENSE)
