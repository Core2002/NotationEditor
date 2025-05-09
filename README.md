# Notation Editor

A simple music notation editor built using the Bevy game engine in Rust. This project allows users to visualize and edit musical notes in a grid-based interface, with support for audio waveform visualization.

## Features

- **Grid-based Note Editing**: Add and remove notes on a grid representing musical tracks and beats.
- **Audio Waveform Visualization**: Render the time-domain graph of an audio file (MP3/WAV) for reference.
- **Interactive Controls**:
  - Scroll to navigate through the grid.
  - Left-click to add notes, right-click to remove notes.
  - Zoom in/out using `Ctrl + Shift + A` and `Ctrl + Shift + B`.
- **Debug Tools**: Logging and camera controls for development.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/) installed on your system.

### Installation

1. Clone the repository:

   ```bash
   git clone https://github.com/Core2002/NotationEditor.git
   cd NotationEditor
   ```

2. Build and run the project:

   ```bash
   cargo run
   ```

### Usage

- **Adding Notes**: Left-click on the grid to place a note.
- **Removing Notes**: Right-click on a note to remove it.
- **Zooming**: Use `Ctrl + Shift + A` to zoom in and `Ctrl + Shift + B` to zoom out.
- **Scrolling**: Use the mouse wheel to scroll vertically.

## Project Structure

- `src/main.rs`: Entry point of the application.
- `src/time_domain_graph.rs`: Handles audio decoding and waveform rendering.
- `src/input.rs`: Manages user input (mouse clicks, scrolling).
- `src/note.rs`: Defines the `Note` component and rendering logic.
- `src/grid.rs`: Manages the grid layout and rendering.
- `src/score.rs`: Tracks and manages musical notes.
- `src/debug_tools.rs`: Provides debugging utilities.

## Dependencies

- [Bevy](https://bevyengine.org/): A data-driven game engine.
- [Symphonia](https://github.com/pdeljanov/Symphonia): For audio decoding.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
