# herdr-pet

English · [简体中文](README.md)

A lightweight desktop companion that animates the current state of local Herdr agents.

## Run

From this directory:

```bash
npm install
npm start
```

Requires Node.js, Rust, and the Tauri 2 system dependencies for your platform. Start a Herdr session first. Select a specific connection using Herdr's existing environment variables:

```bash
HERDR_SESSION=work npm start
HERDR_SOCKET_PATH=/path/to/herdr.sock npm start
```

## Controls

- Drag the pet with the left mouse button using native window dragging.
- Double-click the pet to focus its displayed agent and request activation of the terminal hosting Herdr.
- Hover to see all agents in the selected session, including status, title, and elapsed status time. Each row has separate buttons for focusing the agent and writing a prompt.
- Right-click to select a session or skin, or quit. The session menu appears only when multiple sessions are available. The current skin is Classic pixel.
- The badge counts agents needing attention.

The bubble has a pixel border and tail, readable text, and a scrollable agent list. The top-right settings menu offers language, skin, session, and quit actions. Your language selection is saved locally. On first launch, Chinese system locales select Chinese; other locales select English. The prompt view has a back button at the top left and a send button inside the editor. The bubble has room for a complete agent row or prompt form.

The pet follows the selected session's foreground Herdr client's effective theme, including custom colors and light/dark selection. Bubble surfaces, text, controls, badges, and animation state colors refresh in the background every five seconds. This requires an updated Herdr server and client supporting `client.theme.get`. Older versions, missing compatible clients, and unobserved terminal colors use the pet's defaults. Reading colors does not change your theme.

The prompt editor supports multiple lines and shows its target and character count. Enter inserts a new line; Ctrl/Cmd + Enter sends. Shortcuts do not submit while an input method is composing text. Drafts are kept separately for each session and pane while the app runs, including when you go back or hide the bubble. They are not saved after quitting.

Sending waits for Herdr's API response. While pending, duplicate submission and target changes are disabled. “Submitted to Herdr” confirms submission, not completion of the agent's task. Failure keeps the draft and displays an error for manual retry. There are no automatic retries.

Automatic terminal activation currently has a macOS Ghostty implementation for Herdr running directly in the terminal. macOS may ask for permission to let Herdr control Ghostty. If denied, change it in System Settings → Privacy & Security → Automation. Selecting a pane can succeed while activating its outer window fails; the pet reports the partial success. Precise tmux and GNU Screen window targeting is not supported.

## Window behavior

The transparent, borderless window stays above ordinary windows, remembers its position, and adjusts to display boundaries after dragging. Bubbles prefer the space above the pet and move below it when necessary. Display-layout changes move the pet back into a visible area.

On macOS, the app uses the Accessory activation policy and native window settings to appear across Spaces and alongside full-screen apps. Editing a prompt temporarily lowers its window level so input-method candidate windows can appear above it. Lock screens, secure desktops, and DRM-protected content cannot be covered.

## Performance and validation

Socket requests and subscriptions run in Rust background work; the WebView receives data snapshots. Native dragging avoids IPC on every movement frame. Animation updates every 400 ms; input feedback is event-driven.

```bash
npm test
```

This runs Node tests for draft state, localization, and theme mapping, then the Rust tests. Automated tests do not replace manual checks of native input methods, full-screen behavior, and multiple displays.
