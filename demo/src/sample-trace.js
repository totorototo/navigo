/**
 * Synthetic Alpine route — 25 km, ~80 GPS points.
 *
 * Three qualifying climbs:
 *   1. km  0–8   900 → 1960 m  (~13 % avg grade)
 *   2. km 12–17 1300 → 1820 m  (~10 % avg grade)
 *   3. km 20–23 1450 → 1720 m  (~9 % avg grade)
 *
 * Returns a Float64Array ready to pass to buildTrace():
 *   [lon₀, lat₀, alt₀,  lon₁, lat₁, alt₁, …]
 */
export function generateAlpineRoute() {
  const NUM_POINTS = 80;
  const TOTAL_KM   = 25;

  // Start position — French Alps (Briançon area)
  const START_LON = 6.50;
  const START_LAT = 44.90;
  // Heading ~NE (30° from east), km → degrees
  const KM_PER_DEG_LON = 77;  // at lat 45
  const KM_PER_DEG_LAT = 111;
  const HEADING = Math.PI / 6; // 30°

  function profile(km) {
    if (km < 8)  return 900  + (km / 8)          * 1060; // 900 → 1960 m
    if (km < 12) return 1960 - ((km - 8) / 4)    * 660;  // 1960 → 1300 m
    if (km < 17) return 1300 + ((km - 12) / 5)   * 520;  // 1300 → 1820 m
    if (km < 20) return 1820 - ((km - 17) / 3)   * 370;  // 1820 → 1450 m
    if (km < 23) return 1450 + ((km - 20) / 3)   * 270;  // 1450 → 1720 m
    return              1720 - ((km - 23) / 2)   * 170;  // 1720 → 1550 m
  }

  const flat = new Float64Array(NUM_POINTS * 3);
  for (let i = 0; i < NUM_POINTS; i++) {
    const km  = (i / (NUM_POINTS - 1)) * TOTAL_KM;
    const lon = START_LON + (km * Math.cos(HEADING)) / KM_PER_DEG_LON;
    const lat = START_LAT + (km * Math.sin(HEADING)) / KM_PER_DEG_LAT;
    const alt = profile(km);
    flat[i * 3]     = lon;
    flat[i * 3 + 1] = lat;
    flat[i * 3 + 2] = alt;
  }
  return flat;
}
