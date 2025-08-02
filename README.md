# picoplace: CLI for PCB Layout Synthesis

`picoplace` is a command-line utility for building and synthesizing PCB layouts. It uses the
[Zener](https://github.com/diodeinc/pcb/blob/main/docs/spec.mdx) language to describe
PCB schematics and provides automation on top of KiCad to rapidly generate PCB layouts.

> [!WARNING]
> `picoplace` is under active development. Expect breaking changes and rapid iteration.

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Command Reference](#command-reference)
- [Examples](#examples)
- [Architecture](#architecture)
- [License](#license)

---

## Installation

### From Installer

Follow the instructions [here](https://github.com/diodeinc/pcb/releases/latest)
to install the latest `picoplace`.

### From Source

```bash
# Clone the repository
git clone https://github.com/diodeinc/pcb.git
cd pcb

# Install using the provided script
./install.sh
````

> \[!NOTE]
> Package manager installation coming soon.

### Requirements

* [KiCad 9.x](https://kicad.org/) for layout generation and visualization.

---

## Quick Start

### 1. Create a Design File

Write a file called `blinky.zen`:

```python
load("@stdlib/properties.zen", "Layout")

Resistor = Module("@stdlib/generics/Resistor.zen")
Led = Module("@stdlib/generics/Led.zen")

vcc = Net("VCC")
gnd = Net("GND")
led = Net("LED")

Resistor("R1", value="1kohm", package="0402", P1=vcc, P2=led)
Led("D1", color="red", package="0402", A=led, K=gnd)

Layout("layout", "layout/")
```

### 2. Build Your Design

```bash
picoplace build blinky.zen
```

### 3. Generate Layout

```bash
picoplace layout blinky.zen
```

### 4. Open in KiCad

```bash
picoplace open blinky.zen
```

---

## Core Concepts

Same as original: Components, Nets, Interfaces, Modules, `config()`, and `io()`.

(Refer to original `pcb` README for full code examples.)

---

## Command Reference

### `picoplace build`

Build and validate `.zen` designs.

### `picoplace layout`

Generate layout files from `.zen` files.

### `picoplace open`

Open generated `.kicad_pcb` files in KiCad.

### `picoplace fmt`

Format `.zen` files with `buildifier`.

### `picoplace lsp`

Start the Language Server for editor integration.

---

## Project Structure

```text
my-board/
├── main.zen
├── components/
│   └── resistor.zen
├── modules/
│   └── power_supply.zen
├── eda/
│   ├── symbols/
│   └── footprints/
├── layout/
│   └── main.kicad_pcb
```

---

## Architecture

`picoplace` is a modular Rust workspace:

* **`picoplace-lang`** – Core language support (Zener, Starlark runtime, diagnostics)
* **`picoplace-cli`** – CLI for `build`, `layout`, `fmt`, etc.
* **`picoplace-starlark-lsp`** – LSP server for Zener files
* **`picoplace-ui`** – Terminal UI components (spinner, progress)
* **`picoplace-wasm`** – WebAssembly bindings
* **`picoplace-netlist`** – Netlist and schematic representation
* **`picoplace-kicad-exporter`** – KiCad-compatible layout and symbol exporters
* **`picoplace-buildifier`** – KiCad file parsing, formatters
* **`picoplace-sexpr`** – S-expression support for KiCad file formats
* **`picoplace-core`** – Shared types and utilities

---

## Examples

For full examples, see the `/examples` directory in the repo.

* ✅ LED blink circuit
* ⚙️ Voltage regulator module
* 🧠 Complex system with MCU, sensor, and flash

---

### Third-Party

* Includes [buildifier](https://github.com/bazelbuild/buildtools) under Apache 2.0.
* Built on [starlark-rust](https://github.com/facebookexperimental/starlark-rust).

---
