# Install and update on Windows

Use Windows 11 x64. The installer installs for your Windows account. You do not need Excel, Git, Node.js, Rust or Visual Studio to run the installed application. Microsoft Edge WebView2 is required; the installer handles its setup if it is missing.

## Install the latest build

1. Sign in to GitHub and open [Verify Explorer builds](https://github.com/rados4y/excel-data-explorer/actions/workflows/ci.yml?query=branch%3Amain).
2. Open the latest successful **main** build. Check the commit message to see which changes it includes. Installer builds run automatically after updates to main; allow several minutes for a new build to finish.
3. Under **Artifacts**, download **windows-installer-unsigned**. GitHub requires sign-in to download Actions artifacts.
4. Extract the ZIP and run its `*-setup.exe` installer. Choose installation for your current user.
5. Launch **Excel Data Explorer** from Start. Click **Add Excel** to select a workbook; no configuration file is needed.

There is no published GitHub Release or in-app auto-updater yet. GitHub Actions artifacts are the current installer delivery method and expire according to GitHub's artifact retention policy. If an old artifact expired, use a newer main build or use **Run workflow → main** on the workflow page to produce it again. You need repository write access to run a workflow manually.

The installer is currently unsigned, so Windows SmartScreen may show an unknown-publisher prompt. Verify that you downloaded it from this repository and the intended build before deciding whether to install it.

## Refresh the app after code changes

1. Wait for the new **main** build to finish on the same Actions page.
2. In the running app's Windows tray menu, choose **Exit**. Closing its window with X normally leaves it running in the tray.
3. Download/extract the latest installer and run it over the existing installation. Use the same Windows account and install location; use the reinstall/update option when offered. The app's current product version may still be `0.1.0`, so identify builds by their commit message/SHA on GitHub.
4. Launch the app again from Start.

Spaces, worksheet mappings, saved/nested views, settings and cached data live separately under `%LOCALAPPDATA%\ExcelDataExplorer\`. Installing a newer build does not intentionally reset that directory. Keep it when upgrading or uninstalling, and make backups with the app fully exited. Source workbooks remain at the paths you originally selected.

**Sync now** refreshes imported Excel data. It does not download new application code. Installed app updates currently require the installer steps above.

## Frequent updates while developing

If you want to pull changes directly from GitHub, use a source checkout. This needs Git, Node 22/24 LTS, Rust stable 1.100+, Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK, plus WebView2. Python is needed only to regenerate sample data. See [full Windows build instructions](WINDOWS_RELEASE.md).

First run, from PowerShell:

```powershell
git clone https://github.com/rados4y/excel-data-explorer.git
cd excel-data-explorer
npm ci
npm run desktop
```

To refresh this checkout, stop the development command with Ctrl+C and exit any remaining app instance from the tray, then run from that folder:

```powershell
git pull --ff-only
npm ci
npm run desktop
```

Frontend edits reload through Vite; Rust changes cause Tauri to rebuild/restart. `git pull --ff-only` stops if local commits diverge, rather than overwriting your work. For a normal standalone installation, prefer the installer. To build your own installer, run `npm run desktop:build`; its output is in `src-tauri\target\release\bundle\nsis\`.
