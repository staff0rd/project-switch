# Product Guidelines: project-switch

## UX Personality

project-switch is a **minimal, invisible utility**. It should feel like a native system component — no branding, no chrome, no personality. The tool exists to get out of the user's way as fast as possible. Every design decision should serve speed and function over aesthetics.

## Messaging & Text

All text output follows a **terse, functional** style:

- Single-line messages with no filler words
- State the fact, nothing more (e.g. `no project selected`, `config not found: ~/.project-switch.yml`)
- No tutorials, hints, or suggestions in output — the user is expected to know the tool
- Error messages should name the problem and the relevant value, not explain how to fix it

## Visual Design (Future Windowed UI)

The planned windowed UI must follow **native platform conventions**:

- **Windows:** Align with Windows 11 design language — system fonts, standard window chrome, OS-native controls
- **macOS:** Align with macOS HIG — system fonts, vibrancy/translucency where appropriate, native input handling
- The launcher should be indistinguishable from a built-in OS utility at first glance
- No custom branding, logos, or splash screens

## Interaction Principles

These are non-negotiable and apply to both the current CLI and the future windowed UI:

### Zero Perceived Latency
The launcher must appear and respond to keystrokes instantly. Any visible delay between keypress and UI response breaks the user's flow and violates the core product contract. Performance is a feature, not a goal.

### Keyboard-Only Operation
Every interaction must be completable using only the keyboard. Mouse support may exist but is never required. Tab order, focus management, and keyboard shortcuts must be first-class concerns in every UI decision.

### Minimal Steps to Action
The interaction model is: **hotkey, type, enter**. No menus, no navigation, no intermediate screens, no confirmation dialogs. The fastest path from intent to action is the only acceptable path.

## Configuration Philosophy

- Configuration is a file you edit, not a UI you navigate
- Sensible defaults that require zero configuration for basic usage
- Power features (includes, per-project overrides) are available but never imposed
- The tool never writes to shared/included config files
