//! Simulated Annealing Placer
//!
//! This module implements a simulated annealing algorithm for component placement.
//! It optimizes the placement by minimizing a cost function that considers:
//! - Total wire length (Manhattan distance)
//! - Component overlap
//! - Adherence to AI placement suggestions (if provided)

use crate::{Layout, PlacedComponent, Point, Rect};
use picoplace_netlist::{Instance, InstanceKind, InstanceRef, Schematic};
use std::collections::HashMap;

/// Configuration for the simulated annealing algorithm
#[derive(Debug, Clone)]
pub struct PlacerConfig {
    /// Initial temperature for simulated annealing
    pub initial_temperature: f64,
    /// Cooling rate (0.0 to 1.0)
    pub cooling_rate: f64,
    /// Number of iterations at each temperature
    pub iterations_per_temp: usize,
    /// Minimum temperature to stop the algorithm
    pub min_temperature: f64,
    /// Weight for wire length in the cost function
    pub wire_length_weight: f64,
    /// Weight for component overlap in the cost function
    pub overlap_weight: f64,
    /// Weight for AI hint adherence in the cost function
    pub ai_hint_weight: f64,
}

impl Default for PlacerConfig {
    fn default() -> Self {
        Self {
            initial_temperature: 100.0,
            cooling_rate: 0.95,
            iterations_per_temp: 100,
            min_temperature: 0.1,
            wire_length_weight: 1.0,
            overlap_weight: 10.0,
            ai_hint_weight: 5.0,
        }
    }
}

/// AI placement suggestions
pub type PlacementHints = HashMap<String, Point>;

/// Simulated annealing placer
pub struct SimulatedAnnealingPlacer<'a> {
    schematic: &'a Schematic,
    config: PlacerConfig,
    placement_hints: Option<PlacementHints>,
    board_width: f64,
    board_height: f64,
}

impl<'a> SimulatedAnnealingPlacer<'a> {
    pub fn new(
        schematic: &'a Schematic,
        config: PlacerConfig,
        placement_hints: Option<PlacementHints>,
    ) -> Self {
        Self {
            schematic,
            config,
            placement_hints,
            board_width: 100.0,  // Default board size
            board_height: 100.0,
        }
    }

    /// Run the simulated annealing algorithm
    pub fn run(&mut self) -> Layout<'a> {
        let components: Vec<(&InstanceRef, &Instance)> = self
            .schematic
            .instances
            .iter()
            .filter(|(_inst_ref, inst)| inst.kind == InstanceKind::Component)
            .collect();

        if components.is_empty() {
            return Layout {
                components: vec![],
                width: 100.0,
                height: 100.0,
            };
        }

        // Initialize with grid placement
        let mut current_placement = self.initialize_placement(&components);
        let mut current_cost = self.calculate_cost(&current_placement);

        let mut best_placement = current_placement.clone();
        let mut best_cost = current_cost;

        let mut temperature = self.config.initial_temperature;
        let mut rng = fastrand::Rng::new();

        // Simulated annealing loop
        while temperature > self.config.min_temperature {
            for _ in 0..self.config.iterations_per_temp {
                // Generate a neighbor solution by randomly moving a component
                let mut new_placement = current_placement.clone();
                let len = new_placement.len();
                if let Some(comp) = new_placement.get_mut(rng.usize(0..len)) {
                    // Random perturbation
                    let dx = (rng.f64() - 0.5) * 20.0;
                    let dy = (rng.f64() - 0.5) * 20.0;
                    comp.bounds.x = (comp.bounds.x + dx).max(0.0).min(self.board_width - comp.bounds.width);
                    comp.bounds.y = (comp.bounds.y + dy).max(0.0).min(self.board_height - comp.bounds.height);
                }

                let new_cost = self.calculate_cost(&new_placement);
                let delta_cost = new_cost - current_cost;

                // Accept or reject the new solution
                if delta_cost < 0.0 || rng.f64() < (-delta_cost / temperature).exp() {
                    current_placement = new_placement;
                    current_cost = new_cost;

                    if current_cost < best_cost {
                        best_placement = current_placement.clone();
                        best_cost = current_cost;
                    }
                }
            }

            temperature *= self.config.cooling_rate;
        }

        // Update board dimensions based on final placement
        let (width, height) = self.calculate_board_dimensions(&best_placement);

        Layout {
            components: best_placement,
            width,
            height,
        }
    }

    /// Initialize placement using a simple grid layout
    fn initialize_placement(&self, components: &[(&'a InstanceRef, &'a Instance)]) -> Vec<PlacedComponent<'a>> {
        let num_components = components.len();
        let grid_size = (num_components as f64).sqrt().ceil() as usize;
        let cell_size = 50.0;
        let margin = 20.0;

        let mut placed_components = Vec::new();
        for (i, (instance_ref, instance)) in components.iter().enumerate() {
            let row = i / grid_size;
            let col = i % grid_size;

            let comp_width = 30.0;
            let comp_height = 20.0;

            let mut x = margin + (col as f64 * cell_size);
            let mut y = margin + (row as f64 * cell_size);

            // Use AI hint if available
            if let Some(hints) = &self.placement_hints {
                if let Some(refdes) = &instance.reference_designator {
                    if let Some(hint) = hints.get(refdes) {
                        x = hint.x;
                        y = hint.y;
                    }
                }
            }

            placed_components.push(PlacedComponent {
                instance,
                instance_ref,
                bounds: Rect {
                    x,
                    y,
                    width: comp_width,
                    height: comp_height,
                },
            });
        }

        placed_components
    }

    /// Calculate the cost of a placement
    fn calculate_cost(&self, placement: &[PlacedComponent<'a>]) -> f64 {
        let wire_length_cost = self.calculate_wire_length(placement);
        let overlap_cost = self.calculate_overlap(placement);
        let ai_hint_cost = self.calculate_ai_hint_cost(placement);

        self.config.wire_length_weight * wire_length_cost
            + self.config.overlap_weight * overlap_cost
            + self.config.ai_hint_weight * ai_hint_cost
    }

    /// Calculate total wire length (Manhattan distance)
    fn calculate_wire_length(&self, placement: &[PlacedComponent<'a>]) -> f64 {
        let mut total_length = 0.0;

        // Create a map of component ref -> center position
        let mut positions: HashMap<String, Point> = HashMap::new();
        for comp in placement {
            if let Some(refdes) = &comp.instance.reference_designator {
                positions.insert(
                    refdes.clone(),
                    Point {
                        x: comp.bounds.x + comp.bounds.width / 2.0,
                        y: comp.bounds.y + comp.bounds.height / 2.0,
                    },
                );
            }
        }

        // Calculate wire length for each net
        for net in self.schematic.nets.values() {
            let mut net_positions = Vec::new();
            for port_ref in &net.ports {
                let mut comp_path = port_ref.instance_path.clone();
                if comp_path.pop().is_none() {
                    continue;
                }

                let comp_inst_ref = InstanceRef {
                    module: port_ref.module.clone(),
                    instance_path: comp_path,
                };

                if let Some(comp_instance) = self.schematic.instances.get(&comp_inst_ref) {
                    if let Some(refdes) = &comp_instance.reference_designator {
                        if let Some(pos) = positions.get(refdes) {
                            net_positions.push(*pos);
                        }
                    }
                }
            }

            // Calculate minimum spanning tree length (approximation using star topology)
            if net_positions.len() > 1 {
                let center = self.calculate_centroid(&net_positions);
                for pos in &net_positions {
                    total_length += self.manhattan_distance(&center, pos);
                }
            }
        }

        total_length
    }

    /// Calculate component overlap penalty
    fn calculate_overlap(&self, placement: &[PlacedComponent<'a>]) -> f64 {
        let mut overlap = 0.0;

        for i in 0..placement.len() {
            for j in (i + 1)..placement.len() {
                let rect1 = &placement[i].bounds;
                let rect2 = &placement[j].bounds;

                let x_overlap = (rect1.x + rect1.width).min(rect2.x + rect2.width)
                    - rect1.x.max(rect2.x);
                let y_overlap = (rect1.y + rect1.height).min(rect2.y + rect2.height)
                    - rect1.y.max(rect2.y);

                if x_overlap > 0.0 && y_overlap > 0.0 {
                    overlap += x_overlap * y_overlap;
                }
            }
        }

        overlap
    }

    /// Calculate cost for deviation from AI hints
    fn calculate_ai_hint_cost(&self, placement: &[PlacedComponent<'a>]) -> f64 {
        if let Some(hints) = &self.placement_hints {
            let mut total_deviation = 0.0;

            for comp in placement {
                if let Some(refdes) = &comp.instance.reference_designator {
                    if let Some(hint) = hints.get(refdes) {
                        let center = Point {
                            x: comp.bounds.x + comp.bounds.width / 2.0,
                            y: comp.bounds.y + comp.bounds.height / 2.0,
                        };
                        total_deviation += self.euclidean_distance(&center, hint);
                    }
                }
            }

            total_deviation
        } else {
            0.0
        }
    }

    /// Calculate centroid of a set of points
    fn calculate_centroid(&self, points: &[Point]) -> Point {
        let sum_x: f64 = points.iter().map(|p| p.x).sum();
        let sum_y: f64 = points.iter().map(|p| p.y).sum();
        let n = points.len() as f64;

        Point {
            x: sum_x / n,
            y: sum_y / n,
        }
    }

    /// Calculate Manhattan distance between two points
    fn manhattan_distance(&self, p1: &Point, p2: &Point) -> f64 {
        (p1.x - p2.x).abs() + (p1.y - p2.y).abs()
    }

    /// Calculate Euclidean distance between two points
    fn euclidean_distance(&self, p1: &Point, p2: &Point) -> f64 {
        ((p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sqrt()
    }

    /// Calculate board dimensions based on placement
    fn calculate_board_dimensions(&self, placement: &[PlacedComponent<'a>]) -> (f64, f64) {
        let margin = 20.0;
        let mut max_x: f64 = 0.0;
        let mut max_y: f64 = 0.0;

        for comp in placement {
            max_x = max_x.max(comp.bounds.x + comp.bounds.width);
            max_y = max_y.max(comp.bounds.y + comp.bounds.height);
        }

        (max_x + margin, max_y + margin)
    }
}
