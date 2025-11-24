//! # picoplace-engine
//!
//! This crate contains the core deterministic placement and SVG visualization logic
//! for the PicoPlace hardware design tool. It takes a `Schematic` object as input
//! and produces a placed layout and a corresponding visual representation.
//!
//! This is the heart of the "deterministic core" in the PicoPlace architecture.

use anyhow::{Context, Result};
use picoplace_netlist::{Instance, InstanceKind, InstanceRef, Schematic};
use std::collections::HashMap;
use std::path::Path;
use svg::node::element::{Line, Rectangle, Text};
use svg::Document;

pub mod placer_sa;
pub mod router;

// --- Data Structures ---

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct PlacedComponent<'a> {
    pub instance: &'a Instance,
    pub instance_ref: &'a InstanceRef,
    pub bounds: Rect,
}

#[derive(Debug, Clone)]
pub struct Layout<'a> {
    pub components: Vec<PlacedComponent<'a>>,
    pub width: f64,
    pub height: f64,
}

// --- Placer ---

pub mod placer {
    use super::*;

    /// A very simple grid-based placer.
    pub fn run(schematic: &Schematic) -> Layout {
        let components: Vec<(&InstanceRef, &Instance)> = schematic
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

        let num_components = components.len();
        let grid_size = (num_components as f64).sqrt().ceil() as usize;
        let cell_size = 50.0; // mm
        let margin = 20.0; // mm

        let mut placed_components = Vec::new();
        for (i, (instance_ref, instance)) in components.iter().enumerate() {
            let row = i / grid_size;
            let col = i % grid_size;

            // For now, assume a fixed size for all components
            let comp_width = 30.0;
            let comp_height = 20.0;

            let x = margin + (col as f64 * cell_size);
            let y = margin + (row as f64 * cell_size);

            placed_components.push(PlacedComponent {
                instance, // Pass the reference
                instance_ref, // Pass the reference
                bounds: Rect {
                    x,
                    y,
                    width: comp_width,
                    height: comp_height,
                },
            });
        }

        Layout {
            components: placed_components,
            width: margin * 2.0 + (grid_size as f64 * cell_size),
            height: margin * 2.0 + (grid_size as f64 * cell_size),
        }
    }
}

// --- SVG Generator ---

pub mod svg_generator {
    use super::*;

    /// Generates an SVG document from a layout.
    pub fn run(layout: &Layout, schematic: &Schematic, output_path: &Path) -> Result<()> {
        let mut document = Document::new()
            .set("width", format!("{}mm", layout.width))
            .set("height", format!("{}mm", layout.height))
            .set(
                "viewBox",
                (0, 0, layout.width as u32, layout.height as u32),
            );

        // --- Draw Ratsnest Lines ---
        // Create a map of component ref -> pin positions for easy lookup
        let mut pin_positions: HashMap<String, Point> = HashMap::new();
        for comp in &layout.components {
            // For now, let's just place pins at the center for simplicity
            // A real implementation would parse the footprint to get exact pin locations
            let center_x = comp.bounds.x + comp.bounds.width / 2.0;
            let center_y = comp.bounds.y + comp.bounds.height / 2.0;

            if let Some(refdes) = &comp.instance.reference_designator {
                pin_positions.insert(refdes.clone(), Point { x: center_x, y: center_y });
            }
        }

        for net in schematic.nets.values() {
            let mut points_to_connect = Vec::new();
            for port_ref in &net.ports {
                // Find the parent component of this port
                let mut comp_path = port_ref.instance_path.clone();
                if comp_path.pop().is_none() {
                    continue;
                }

                let comp_inst_ref = picoplace_netlist::InstanceRef {
                    module: port_ref.module.clone(),
                    instance_path: comp_path,
                };

                // Find the instance in the schematic that matches this reference
                if let Some(comp_instance) = schematic.instances.get(&comp_inst_ref) {
                    if let Some(refdes) = &comp_instance.reference_designator {
                        if let Some(pos) = pin_positions.get(refdes) {
                            points_to_connect.push(pos);
                        }
                    }
                }
            }

            if points_to_connect.len() > 1 {
                for i in 0..points_to_connect.len() - 1 {
                    let p1 = points_to_connect[i];
                    let p2 = points_to_connect[i + 1];
                    let line = Line::new()
                        .set("x1", p1.x)
                        .set("y1", p1.y)
                        .set("x2", p2.x)
                        .set("y2", p2.y)
                        .set("stroke", "gray")
                        .set("stroke-width", 0.2);
                    document = document.add(line);
                }
            }
        }

        // --- Draw Components ---
        for comp in &layout.components {
            let rect = Rectangle::new()
                .set("x", comp.bounds.x)
                .set("y", comp.bounds.y)
                .set("width", comp.bounds.width)
                .set("height", comp.bounds.height)
                .set("fill", "lightblue")
                .set("stroke", "blue")
                .set("stroke-width", 0.5);

            document = document.add(rect);

            if let Some(refdes) = &comp.instance.reference_designator {
                let text = Text::new()
                    .set("x", comp.bounds.x + 2.0)
                    .set("y", comp.bounds.y + 5.0)
                    .set("font-size", "4px")
                    .add(svg::node::Text::new(refdes));
                document = document.add(text);
            }
        }

        svg::save(output_path, &document)
            .with_context(|| format!("Failed to save SVG to {}", output_path.display()))?;

        Ok(())
    }
}