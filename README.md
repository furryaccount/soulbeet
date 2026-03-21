# Soulbeet

[![Docker Pulls](https://img.shields.io/docker/pulls/docccccc/soulbeet)](https://hub.docker.com/repository/docker/docccccc/soulbeet/general)
[![Docker Image Size](https://img.shields.io/docker/image-size/docccccc/soulbeet)](https://hub.docker.com/repository/docker/docccccc/soulbeet/general)
[![Docker Image Version](https://img.shields.io/docker/v/docccccc/soulbeet)](https://hub.docker.com/repository/docker/docccccc/soulbeet/general)

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/terry90/soulbeet/image-build-push.yml)](https://github.com/terry90/soulbeet/actions)
[![GitHub License](https://img.shields.io/github/license/terry90/soulbeet)](https://github.com/terry90/soulbeet)
[![GitHub Repo stars](https://img.shields.io/github/stars/terry90/soulbeet)](https://github.com/terry90/soulbeet)

Soulbeet is a modern, self-hosted music downloader and manager. It bridges the gap between Soulseek (via `slskd`) and your music library (managed by `beets`), providing a seamless flow from search to streaming-ready library.

Screenshots: [here](./screenshots)

## Features

-   **Unified Search**: Search for albums and tracks using MusicBrainz or Last.fm metadata, then find sources on Soulseek.
-   **Multiple Metadata Providers**: Choose between MusicBrainz (better for albums) or Last.fm (better for single tracks) in your user settings.
-   **One-Click Download & Import**: Select an album (or just some tracks), choose your target folder, and Soulbeet handles the rest.
-   **Automated Importing**: Automatically monitors downloads and uses the `beets` CLI to tag, organize, and move files to your specified music folder.
-   **Music Discovery**: Automatic music recommendations powered by Last.fm and ListenBrainz. The engine builds a profile of your taste, generates candidates through 7 independent signals (track similarity, artist chains, collaborative filtering, tag exploration, momentum detection, Troi playlists, artist radio), and downloads them into Navidrome playlists. Three discovery profiles (Conservative, Balanced, Adventurous) let you control how far from your comfort zone you want to go.
-   **Navidrome Integration**: Discovery playlists sync directly to Navidrome. Rate tracks as you listen -- liked tracks get promoted into your library, disliked tracks get cleaned up automatically.
-   **User Management**: Multi-user support with private folders and shared folders for families/friend groups. Each user has their own discovery profiles, listening history, and preferences.

## Architecture

1.  **Soulbeet Web**: The main interface (Dioxus Fullstack).
2.  **Slskd**: The Soulseek client backend. Soulbeet communicates with `slskd` to initiate and monitor downloads.
3.  **Beets**: The music library manager. Soulbeet executes `beet import` to process finished downloads.
4.  **Navidrome**: Music streaming server. Soulbeet creates discovery playlists and syncs ratings for automatic track promotion/removal.
5.  **Last.fm / ListenBrainz**: Scrobble services used by the recommendation engine to build user taste profiles and generate discovery candidates.
6.  **SQLite**: Stores user accounts, folder configurations, discovery candidates, engine reports, and listening profiles.

## Self-Hosting with Docker

The recommended way to run Soulbeet is via Docker Compose. This ensures all dependencies (like `beets` and `python`) are correctly set up.

**Compatibility:** The Docker image supports both **AMD64** and **ARM64** architectures.

### Prerequisites

-   Docker & Docker Compose (or podman-compose)

### Quick Start

1.  Create a `docker-compose.yml` file:

```yaml
services:
  soulbeet:
    image: docker.io/docccccc/soulbeet:latest
    restart: unless-stopped
    ports:
      - 9765:9765
    environment:
      - DATABASE_URL=sqlite:/data/soulbeet.db
      # The path where slskd saves files (INSIDE the soulbeet container)
      - DOWNLOAD_PATH=/downloads
      # Optional: Beets configuration
      - BEETS_CONFIG=/config/config.yaml
      - SECRET_KEY=secret
    volumes:
      # Data persistence (DB)
      - ./data:/data
      # Map the SAME download folder slskd uses
      - /path/to/slskd/downloads:/downloads
      # Map your music libraries (where beets will move files to)
      - /path/to/music:/music
    # Optional
    depends_on:
      - slskd

  # Optional
  # Example slskd service if you don't have one running
  slskd:
    image: slskd/slskd
    environment:
      - SLSKD_REMOTE_CONFIGURATION=true
    volumes:
      - ./slskd-config:/app/slskd.conf.d
      - /path/to/slskd/downloads:/app/downloads
    ports:
      - "5030:5030"
```

2.  **Important**: The `/downloads` volume must match between `slskd` and `soulbeet` so Soulbeet can see the files `slskd` downloaded.

3.  Build and Run:

```bash
docker-compose up -d --build
```

### Initial Setup

1.  Open `http://localhost:9765`
2.  Login with the default credentials:
    -   Username: `admin`
    -   Password: `admin`
3.  Go to **Settings**.
4.  **Change your password** (Create a new user if you prefer and delete the admin later, or just change the admin logic if you forked the code).
5.  **Configure slskd connection** (Settings > Config): Add your slskd URL (e.g., `http://slskd:5030`) and API key. Get your API key from slskd config file or [add one](https://github.com/slskd/slskd/blob/master/docs/config.md#yaml-24).
6.  **Add Music Folders** (Settings > Library): Add the paths where you want your music to be stored (e.g., `/music/Person1`, `/music/Person2`,  `/music/Shared`). These must be paths accessible inside the Docker container.
7.  **Configure Search Preferences** (Settings > Search, optional): Choose your default metadata provider. If you want to use Last.fm, first add your API key in Settings > Config.

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | Connection string for SQLite | `sqlite:soulbeet.db` |
| `DOWNLOAD_PATH` | Path where downloads are saved | `/downloads` |
| `BEETS_CONFIG` | Path to custom beets config file | `beets_config.yaml` |
| `BEETS_ALBUM_MODE` | Enable album import mode (see below) | `false` |
| `SECRET_KEY` | Used to encrypt tokens and Navidrome credentials | |
| `NAVIDROME_URL` | Navidrome server URL (for discovery playlists and rating sync) | |

**Note**: slskd URL and API key are configured through the web UI (Settings > Config) and stored in the database.

### Beets Configuration

Soulbeet uses `beets` to import music. You can mount a custom `config.yaml` to `/config/config.yaml` (or wherever you point `BEETS_CONFIG` to) to customize how beets behaves (plugins, naming formats, etc.).

Default `beet import` flags used:
-   `-q`: Quiet mode (no user interaction)
-   `-s`: Singleton mode (Default behavior unless `BEETS_ALBUM_MODE` is set)
-   `-l [library_path]`: Library database path (per-folder)
-   `-d [target_path]`: Import to the specific folder selected in the web UI.

### Library Management

**Important**: Each music folder you configure in Soulbeet has its own beets database (`.beets_library.db`) stored at the root of that folder. This enables:

- Per-folder duplicate detection
- Independent library management for each user/folder
- Cross-library duplicate detection

#### Interacting with Your Library

In soulbeet each user can have multiple libraries. Each library is a folder that contains music files and a `.beets_library.db` file. This database is used to avoid duplicate tracks within the same library.

Since we use different databases, we can't directly compare tracks across libraries. However, we can use the `beets` CLI to interact with each library individually. This way you can add tracks outside of soulbeet but keep them in sync with your library.

To manually interact with a library (list tracks, modify tags, remove items, etc.), use the `beet` CLI with the `-l` flag pointing to the folder's database:

```bash
# List all tracks in a library
beet -l /music/Person1/.beets_library.db ls
```

If using Docker, run these commands inside the container:
```bash
docker exec -it <container_name> beet -l /music/Person1/.beets_library.db ls
```

For more beets commands, see the [beets documentation](https://beets.readthedocs.io/en/stable/reference/cli.html).

#### Album Mode (`BEETS_ALBUM_MODE`)

By setting `BEETS_ALBUM_MODE=true`, Soulbeet will attempt to group downloaded files by their parent directory and import them as an album instead of singletons.

This flag is only needed when you want album tags on your tracks. E.g: `albumartist` `mb_albumid` and so on..

**Important for Album Mode:**
Since `beets` is run in quiet mode (`-q`), it will skip any imports that require user intervention (e.g., if the metadata doesn't match confidently). To ensure partial albums or less popular releases are imported correctly, you **must** configure your `beets_config.yaml` to be more permissive.

Recommended additions to your `beets_config.yaml` for Album Mode:

```yaml
import:
  # quiet_fallback: asis # Optional: If no strong match, import with existing tags instead of skipping
  timid: no

match:
  strong_rec_thresh: 0.10  # Lower threshold to accept less confident matches
  max_rec:
    missing_tracks: strong
    unmatched_tracks: strong
  distance_weights:
    missing_tracks: 0.1  # Reduce weight (I think default is 0.9) to penalize less for missing tracks, improving the overall score.
    # unmatched_tracks: 0.2  # Optional: Similar for extra tracks.
```

*Note: Tweaking `strong_rec_thresh` and other matching parameters increases the risk of incorrect tags, but is necessary for fully automated imports of obscure or partial albums.*

## Development

1.  Install Rust and `dioxus_cli`.
2.  Run the tailwind watcher:
    ```bash
    ./css.sh
    ```
3.  Run the app:
    ```bash
    dx serve --platform web

## TODO & Ideas

- Mobile app (nothing much to do honestly)
- Enhance the default beets configuration
- Find a way to avoid album dups ? e.g `Clair Obscur_ Expedition 33 (Original Soundtrack)` & `Clair Obscur_ Expedition 33_ Original Soundtrack` - Rare but annoying
- Add play preview on album track list
- Improve slskd search. Currently:
  - Single track search, query: "{artist} {track_title}" -> more resilient
  - Multiple tracks search, query: "{artist} {album}" -> best for metadata and grouping tracks by album
- Complete library manager, removal of tracks
- Synchronize a playlist (Spotify or other)
- Populate release_year from MusicBrainz to activate the new-release boost in discovery
