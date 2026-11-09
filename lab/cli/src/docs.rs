//! What each stage and parameter does, for the published schema. Kept out of the core so the
//! WebAssembly module doesn't carry it; a test checks it covers the core's `STAGES` exactly.

pub const RECIPE: &str = "A map lab recipe: a pipeline of stages plus a seed. Building it gives the same map on every platform, tile for tile.";

/// (stage, what it does, the talk's demo it comes from)
pub const STAGES: &[(&str, &str, &str)] = &[
    ("rooms", "Generator. Random rectangular rooms, each kept if it doesn't touch an earlier one, then sorted left to right and joined in that order by L-shaped corridors.", "c01-rooms"),
    ("bsp", "Generator. Rooms placed inside ever-smaller quarters of the map: each attempt picks a rectangle from a growing list, tries a room near its corner, and on success adds that rectangle's four quarters to the list. Then sorted and joined like rooms.", "c02-bsprooms"),
    ("cellular", "Generator. Random noise, then smoothing passes: a tile becomes wall with five or more wall neighbours of eight, or none, and floor otherwise. The outer ring keeps its noise.", "c03-cellular"),
    ("drunkard", "Generator. Diggers that stagger at random, the first from the centre and each later one from a random floor tile, until a share of the map is floor.", "c04-drunkard"),
    ("dla", "Generator. Diffusion-limited aggregation from a small cross at the centre, until a share of the map is floor: particles from random points wander (walk), fly straight at the centre (attractor), or fly straight and land mirrored about the centre column (symmetric), and stick where they meet floor.", "c05-dla, c06-dla-attractor, c07-dla-symmetry"),
    ("dla-erode", "Modifier. Particles dropped on random floor tiles wander until they leave the floor and carve the wall tile they reach.", "c08-dla-erosion"),
    ("voronoi", "Generator. Random seed points; every tile belongs to its nearest seed; walls where neighbouring tiles belong to different seeds, and around the edge.", "c08a-voronoi"),
    ("noise", "Generator. Fractal simplex noise (fBm): a grey-scale height map (height), thresholded into water, grass and mountain (overworld), redrawn as the sampling scale shrinks (zoom), or two noises blended as the zoom deepens (double). Every noise stage of a recipe samples the same noise, from the recipe's seed.", "c10-perlin, c11-overworld, c12-noisezoom, c13-double-noise"),
    ("combine", "Generator. Each input pipeline runs on a fresh map; the result takes the left half from the first and the right half from the second.", "c09-combination"),
    ("prefab", "Modifier. Stamps a prefab: the vault (treasure ringed by traps, 6 by 5) in a random room big enough for it, on any open 7-by-6 patch of floor, or at the centre; or the spine (our own 11-wide hall, standing in for the talk's) down the middle of the map.", "c09-combination, c14-not-a-trap, c15-still-not-a-trap"),
    ("cull", "Analysis. Finds the open tile nearest the centre and marks, or removes, every walkable tile that can't be walked to from it.", "c16-unreachable, c17-starting-left"),
    ("start", "Placement. The start: the open tile nearest the middle of the left edge, or nearest the centre.", "c17-starting-left, c19-ending-dijkstra"),
    ("exit", "Placement. The exit: the open tile nearest the middle of the right edge, or the reachable tile farthest from the start by walking distance; then the shortest path from the start to it.", "c18-ending-right, c19-ending-dijkstra"),
    ("hot-path", "Analysis. Walking distance from the start-to-exit path: tiles within the radius are on the hot path; with cull set, every tile that far or farther from the path, other than the path, becomes wall.", "c20-hot-path"),
    ("hot-rooms", "Analysis. Marks the rooms the start-to-exit path passes through (inside a room or on the row below it), and the rest.", "c21-hot-rooms"),
];

/// (stage, parameter, what it does)
pub const PARAMS: &[(&str, &str, &str)] = &[
    ("rooms", "attempts", "Rooms tried; each one that overlaps an earlier room (touching counts) is discarded."),
    ("rooms", "min_size", "Smallest room side, in the talk's rectangle units: a side of n covers n + 1 tiles."),
    ("rooms", "max_size", "Largest room side, inclusive, in the same units."),
    ("bsp", "attempts", "Room placements tried."),
    ("cellular", "wall_percent", "The chance, in percent, that a tile starts as wall."),
    ("cellular", "iterations", "Smoothing passes."),
    ("drunkard", "lifetime", "Steps a digger takes before it stops, less one: it digs lifetime + 1 tiles unless it leaves the map."),
    ("drunkard", "floor_divisor", "Digging stops once floor covers at least 1 / floor_divisor of the map."),
    ("dla", "mode", "How particles move: walk at random, fly straight at the centre, or fly straight and land mirrored."),
    ("dla", "floor_divisor", "Particles stop once floor covers at least 1 / floor_divisor of the map."),
    ("dla-erode", "iterations", "Particles dropped."),
    ("voronoi", "seeds", "Seed points, one cell each."),
    ("voronoi", "metric", "The distance measure that decides the walls. The step-through shows all three."),
    ("noise", "mode", "What the noise becomes: an overworld, a height map, a zoom, or a double-noise zoom."),
    ("noise", "octaves", "Layers of noise summed, each finer than the last."),
    ("noise", "gain", "How much each octave counts relative to the one before."),
    ("noise", "lacunarity", "How much finer each octave is than the one before."),
    ("noise", "frequency", "How fine the first octave is."),
    ("combine", "join", "How the inputs are joined: the left half of the first and the right half of the second."),
    ("combine", "inputs", "Two pipelines, run in order on fresh maps of the recipe's size."),
    ("prefab", "shape", "The prefab: the vault, or the spine."),
    ("prefab", "place", "Where it goes: in a room that fits it, on any open patch it fits, or at the centre. The spine only goes at the centre."),
    ("cull", "from", "Where reachability is measured from: the open tile nearest the centre."),
    ("cull", "mode", "Remove unreachable floor (make it wall), or only mark it."),
    ("start", "at", "Where the start goes."),
    ("exit", "at", "Where the exit goes."),
    ("hot-path", "radius", "Tiles fewer than this many steps from the path are on the hot path."),
    ("hot-path", "cull", "If above zero, tiles this many steps or more from the path, other than the path, become wall."),
];
