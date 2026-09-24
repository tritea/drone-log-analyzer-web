/** Map domain: base-map notices, map cache panel, map view and map-state store. */
export default {
  notice: {
    loading: 'Loading base map...',
    noGeo: 'Current track has no GPS coordinates; track and waypoints cannot be overlaid.',
  },
  view: {
    sourceFailed: 'The current base-map source failed to load. Switch to another source at the top right.',
  },
  controls: {
    locate: 'Locate',
    locateTitle: 'Locate the drone at its current position',
    providerTitle: 'Base-map source',
    terrain: 'Terrain',
    terrainTitle: 'Enable DEM terrain',
    terrainUnsupported: 'The current base map does not support terrain',
    lock: 'Lock',
    lockTitle: 'Lock the follow view',
    models: 'Models',
    modelsTitle: 'Manage custom 3D models',
    tilesBtn: '3D Tiles',
    tilesTitle: 'Manage 3D Tiles survey models',
    unfold: 'Expand controls',
    fold: 'Collapse controls',
  },
}
