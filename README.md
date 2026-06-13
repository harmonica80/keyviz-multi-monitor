# Keyviz 鍵盤按鍵顯示器（支援多螢幕）

<div>
   <img src="https://img.shields.io/github/v/release/harmonica80/keyviz-multi-monitor?style=flat-square" alt="Releases">
   <img src="https://img.shields.io/github/downloads/harmonica80/keyviz-multi-monitor/total?style=flat-square" alt="Downloads">
   <img src="https://img.shields.io/github/stars/harmonica80/keyviz-multi-monitor?style=flat-square" alt="Stars">
   <img src="https://img.shields.io/github/license/harmonica80/keyviz-multi-monitor?style=flat-square" alt="License">
   <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey?style=flat-square" alt="Platform Support">
</div>

Keyviz is a **free and open-source** tool that visualizes your keypresses and mouse actions in real-time. Let your audience know what handy shortcuts you're pressing during tutorials, presentations, collaborations, or whenever you need.

This portable build also includes an English / Traditional Chinese language
switch for the settings interface.

On Windows, the visualization uses a compact, click-through overlay that grows
only to the size of the displayed keycaps instead of covering an entire monitor.

## ⌨️ Keypress & 🖱️ Mouse Actions
Along with normal keys, you can also visualize mouse actions like <kbd>Cmd</kbd> + <kbd>Click</kbd>, <kbd>Alt</kbd> + <kbd>Drag</kbd>, etc.

<img src="previews/visualization.png" alt="Keystroke Visualization" width="450">

Display mouse clicks and scroll wheel movements alongside your cursor.

<img src="previews/mouse-indicator.gif" alt="Mouse Indicator" width="450">

</br>

## ⚙️ Full Customization
Don't settle for defaults. Every aspect of the visualization is under your control:
- **Styling:** Change colors (modifier vs. regular keys), size, layout, border, and background.
- **Filtering:** Control which keys are shown using hotkey or custom filters.
- **History:** Keep a visual trail of your recent inputs.
- **Position:** Move the visualization to any part of your screen.
- **Animations:** Customize how inputs appear and disappear with preset entry and exit animations.

</br>

<img src="previews/settings.png" alt="Settings Panel" width="600">

</br>

## 📥 Installation

### Windows & macOS
You can download the latest version of Keyviz from the **[GitHub Releases](https://github.com/harmonica80/keyviz-multi-monitor/releases)** page.

*   **Windows:** Download the `.msi` installer, run it, and follow the steps.
*   **macOS:** Download the `.dmg`. 
    **Note:** Keyviz requires **Input Monitoring** and **Accessibility** permissions. Enable them here:
    `Settings > Privacy & Security > Input Monitoring & Accessibility`

### Linux (x11)
Keyviz is compatible with Linux using the X11 protocol. Currently, you can try it out by following the build instructions below.

</br>

## 🛠️ Build Instructions

If you want to contribute or build the latest features from the source, ensure you have [Node.js](https://nodejs.org/) and [Tauri](https://v2.tauri.app/start) set up on your system.

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/harmonica80/keyviz-multi-monitor.git
    cd keyviz-multi-monitor
    ```

2.  **Install dependencies:**
    ```bash
    npm install
    ```

3.  **Build the executable:**
    ```bash
    npx tauri build
    ```

4.  **Build a portable Windows executable without an installer:**
    ```bash
    npm run build:portable
    ```
    The single executable is written to
    `src-tauri/target/release/keyviz.exe`. See [PORTABLE.md](PORTABLE.md) for
    details.

<br/>


## 💖 Support the Project

*   **Star the Repo:** It helps others discover the project!
*   **GitHub Sponsors:** [Sponsor @mularahul](https://github.com/sponsors/mulaRahul)
*   **Keyviz Pro:** Get access to exclusive features while supporting the development of this open-source project.

👉 **[Upgrade to Pro at keyviz.org/pro](https://keyviz.org/pro)**

</br>

---

  Built with 🦀 and ❤️ using <a href="https://v2.tauri.app/">Tauri</a>.
