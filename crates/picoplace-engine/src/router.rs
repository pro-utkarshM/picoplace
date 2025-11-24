//! A* Router
//!
//! This module implements an A* search algorithm for routing nets on a PCB.
//! It routes nets on a grid while avoiding obstacles (components).

use crate::{Layout, Point, Rect};
use picoplace_netlist::Schematic;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

/// Grid cell coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCell {
    pub x: i32,
    pub y: i32,
}

/// A node in the A* search
#[derive(Debug, Clone, PartialEq)]
struct AStarNode {
    cell: GridCell,
    g_cost: f64, // Cost from start
    h_cost: f64, // Heuristic cost to goal
    parent: Option<GridCell>,
}

impl AStarNode {
    fn f_cost(&self) -> f64 {
        self.g_cost + self.h_cost
    }
}

impl Eq for AStarNode {}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.f_cost().partial_cmp(&self.f_cost()).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A routed path
#[derive(Debug, Clone)]
pub struct RoutedPath {
    pub net_name: String,
    pub points: Vec<Point>,
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Grid resolution (mm per cell)
    pub grid_resolution: f64,
    /// Penalty for routing near components
    pub component_penalty: f64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            grid_resolution: 1.0,
            component_penalty: 5.0,
        }
    }
}

/// A* Router
pub struct AStarRouter<'a> {
    schematic: &'a Schematic,
    layout: &'a Layout<'a>,
    config: RouterConfig,
    routing_priorities: Vec<String>,
    grid_width: i32,
    grid_height: i32,
    obstacles: HashSet<GridCell>,
}

impl<'a> AStarRouter<'a> {
    pub fn new(
        schematic: &'a Schematic,
        layout: &'a Layout<'a>,
        config: RouterConfig,
        routing_priorities: Vec<String>,
    ) -> Self {
        let grid_width = (layout.width / config.grid_resolution).ceil() as i32;
        let grid_height = (layout.height / config.grid_resolution).ceil() as i32;

        let mut router = Self {
            schematic,
            layout,
            config,
            routing_priorities,
            grid_width,
            grid_height,
            obstacles: HashSet::new(),
        };

        router.initialize_obstacles();
        router
    }

    /// Initialize obstacles based on component positions
    fn initialize_obstacles(&mut self) {
        for comp in &self.layout.components {
            let x_start = (comp.bounds.x / self.config.grid_resolution).floor() as i32;
            let y_start = (comp.bounds.y / self.config.grid_resolution).floor() as i32;
            let x_end = ((comp.bounds.x + comp.bounds.width) / self.config.grid_resolution).ceil() as i32;
            let y_end = ((comp.bounds.y + comp.bounds.height) / self.config.grid_resolution).ceil() as i32;

            for x in x_start..=x_end {
                for y in y_start..=y_end {
                    self.obstacles.insert(GridCell { x, y });
                }
            }
        }
    }

    /// Route all nets
    pub fn route(&self) -> Vec<RoutedPath> {
        let mut routed_paths = Vec::new();

        // Create a map of component ref -> center position
        let mut component_positions: HashMap<String, Point> = HashMap::new();
        for comp in &self.layout.components {
            if let Some(refdes) = &comp.instance.reference_designator {
                component_positions.insert(
                    refdes.clone(),
                    Point {
                        x: comp.bounds.x + comp.bounds.width / 2.0,
                        y: comp.bounds.y + comp.bounds.height / 2.0,
                    },
                );
            }
        }

        // Route nets in priority order
        let mut nets_to_route: Vec<_> = self.schematic.nets.iter().collect();
        
        // Sort by priority if provided
        if !self.routing_priorities.is_empty() {
            nets_to_route.sort_by_key(|(net_name, _)| {
                self.routing_priorities
                    .iter()
                    .position(|p| p == *net_name)
                    .unwrap_or(usize::MAX)
            });
        }

        for (net_name, net) in nets_to_route {
            let mut net_positions = Vec::new();

            for port_ref in &net.ports {
                let mut comp_path = port_ref.instance_path.clone();
                if comp_path.pop().is_none() {
                    continue;
                }

                let comp_inst_ref = picoplace_netlist::InstanceRef {
                    module: port_ref.module.clone(),
                    instance_path: comp_path,
                };

                if let Some(comp_instance) = self.schematic.instances.get(&comp_inst_ref) {
                    if let Some(refdes) = &comp_instance.reference_designator {
                        if let Some(pos) = component_positions.get(refdes) {
                            net_positions.push(*pos);
                        }
                    }
                }
            }

            if net_positions.len() > 1 {
                // Route using minimum spanning tree approach
                let path = self.route_net(&net_positions);
                routed_paths.push(RoutedPath {
                    net_name: net_name.clone(),
                    points: path,
                });
            }
        }

        routed_paths
    }

    /// Route a single net connecting multiple points
    fn route_net(&self, positions: &[Point]) -> Vec<Point> {
        let mut path = Vec::new();
        
        if positions.is_empty() {
            return path;
        }

        // Use a simple star topology: route from first point to all others
        let start = positions[0];
        path.push(start);

        for target in &positions[1..] {
            if let Some(segment) = self.find_path(start, *target) {
                path.extend(segment);
            }
        }

        path
    }

    /// Find a path between two points using A*
    fn find_path(&self, start: Point, goal: Point) -> Option<Vec<Point>> {
        let start_cell = self.point_to_grid(start);
        let goal_cell = self.point_to_grid(goal);

        let mut open_set = BinaryHeap::new();
        let mut closed_set = HashSet::new();
        let mut came_from: HashMap<GridCell, GridCell> = HashMap::new();
        let mut g_scores: HashMap<GridCell, f64> = HashMap::new();

        g_scores.insert(start_cell, 0.0);
        open_set.push(AStarNode {
            cell: start_cell,
            g_cost: 0.0,
            h_cost: self.heuristic(start_cell, goal_cell),
            parent: None,
        });

        while let Some(current) = open_set.pop() {
            if current.cell == goal_cell {
                // Reconstruct path
                return Some(self.reconstruct_path(&came_from, current.cell));
            }

            if closed_set.contains(&current.cell) {
                continue;
            }

            closed_set.insert(current.cell);

            // Explore neighbors
            for neighbor in self.get_neighbors(current.cell) {
                if closed_set.contains(&neighbor) {
                    continue;
                }

                let movement_cost = if self.obstacles.contains(&neighbor) {
                    self.config.component_penalty
                } else {
                    1.0
                };

                let tentative_g_score = g_scores.get(&current.cell).unwrap_or(&f64::INFINITY) + movement_cost;

                if tentative_g_score < *g_scores.get(&neighbor).unwrap_or(&f64::INFINITY) {
                    came_from.insert(neighbor, current.cell);
                    g_scores.insert(neighbor, tentative_g_score);

                    open_set.push(AStarNode {
                        cell: neighbor,
                        g_cost: tentative_g_score,
                        h_cost: self.heuristic(neighbor, goal_cell),
                        parent: Some(current.cell),
                    });
                }
            }
        }

        None // No path found
    }

    /// Get neighboring cells
    fn get_neighbors(&self, cell: GridCell) -> Vec<GridCell> {
        let mut neighbors = Vec::new();
        let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];

        for (dx, dy) in directions {
            let new_x = cell.x + dx;
            let new_y = cell.y + dy;

            if new_x >= 0 && new_x < self.grid_width && new_y >= 0 && new_y < self.grid_height {
                neighbors.push(GridCell { x: new_x, y: new_y });
            }
        }

        neighbors
    }

    /// Heuristic function (Manhattan distance)
    fn heuristic(&self, a: GridCell, b: GridCell) -> f64 {
        ((a.x - b.x).abs() + (a.y - b.y).abs()) as f64
    }

    /// Convert point to grid cell
    fn point_to_grid(&self, point: Point) -> GridCell {
        GridCell {
            x: (point.x / self.config.grid_resolution).round() as i32,
            y: (point.y / self.config.grid_resolution).round() as i32,
        }
    }

    /// Convert grid cell to point
    fn grid_to_point(&self, cell: GridCell) -> Point {
        Point {
            x: cell.x as f64 * self.config.grid_resolution,
            y: cell.y as f64 * self.config.grid_resolution,
        }
    }

    /// Reconstruct path from came_from map
    fn reconstruct_path(&self, came_from: &HashMap<GridCell, GridCell>, mut current: GridCell) -> Vec<Point> {
        let mut path = vec![self.grid_to_point(current)];

        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(self.grid_to_point(current));
        }

        path.reverse();
        path
    }
}
