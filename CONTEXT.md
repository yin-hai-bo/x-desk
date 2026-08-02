# Glossary

- **Dock**: A desktop-mounted container window used to host wallpaper content behind desktop icons.
- **Desktop Rebuild Trigger**: A Windows event that indicates desktop wallpaper host windows may need refresh or full reconstruction.
- **Video Host**: A child window inside a Dock that is used as the Media Foundation video render target.
- **Video Wallpaper**: A local video rendered behind desktop icons as desktop background content.
- **Wallpaper Reset**: Recreating desktop host discovery and Dock windows from current config after shell or desktop handles become invalid.

# Runtime Behavior

- Desktop rebuild triggers include WorkerW destruction, taskbar recreation, session unlock, and display changes.
- A wallpaper refresh reuses the current Desktop and Dock windows when possible, applying the current config to each monitor.
- A wallpaper reset drops cached Desktop and Dock windows, rediscovers the desktop host windows, and rebuilds wallpapers from the current config.

# Win32 Window Binding

- `Window<T>` stores its pointer in `GWLP_USERDATA` during `WM_NCCREATE` and clears the binding during `WM_NCDESTROY`.
- `Window<T>::get_self_from_hwnd` returns `NonNull<Window<T>>`; callers must perform any dereference explicitly in `unsafe` code and ensure the HWND belongs to the requested `Window<T>` type without overlapping borrows.
