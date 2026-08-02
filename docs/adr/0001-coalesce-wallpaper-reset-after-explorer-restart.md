# Coalesce wallpaper reset after Explorer restart

When Explorer restarts, the old WorkerW can be destroyed before the new Progman window exists, so an immediate Wallpaper Reset can fail even though the desktop will become recoverable shortly after. WorkerW destruction therefore schedules a short delayed reset, while TaskbarCreated is treated as the stronger desktop rebuild signal: it cancels any pending WorkerW reset, recreates the tray icon, and resets wallpapers immediately to avoid duplicate reconstruction.
