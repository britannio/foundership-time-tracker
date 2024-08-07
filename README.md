# Foundership Time Tracker

I need to log how much time I spend in the office for the next few weeks.
This Tauri application sits in the background, periodically running `$ networksetup -getairportnetwork en0` to check
if the connected Wi-Fi network matches the expected one and logs it to SQLite if so.