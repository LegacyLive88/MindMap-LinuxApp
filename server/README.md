# MindMap cloud

This is the endpoint the desktop program syncs with. Put it on a public address such as `https://maps.example.com`. Sign in there in a browser to edit the same maps, and use that address, username, and password in the desktop gear menu.

The app is Next.js. It stores accounts, the current map, and backups in MariaDB. On a cPanel account the public site is usually Apache behind the WHM nginx layer, so `cpanel/` is a PHP proxy that uses Guzzle to forward the domain to Next.js on `127.0.0.1`.

## What it does

- `POST /api/v1/login` checks the username and password and returns a token.
- `GET /api/v1/sync` and `POST /api/v1/sync` download or upload the whole map. The desktop sends the revision it last synced. If the cloud has moved on, the upload is refused and the cloud copy is returned so the program can ask which one to keep.
- An accepted upload writes the previous map into `backups` first. Automatic backups keep the latest 40. Backups you take by hand are kept.
- The browser at `/` lists canvases. `/map/<id>` edits a canvas. `/backups` lists, downloads, and restores backups.
- `/setup` creates the first account, then stops working.

Send `Authorization: Bearer <token>` from the desktop or a future mobile app. The browser uses an HttpOnly cookie set by login.

## 1. MariaDB

In cPanel, create a MariaDB database and a user with all rights on that database. From the `server` directory:

```bash
cp .env.example .env
```

Edit `.env` with the host (usually `localhost`), database, user, and password. Then:

```bash
npm install
npm run setup
npm run create-user -- ada 'a long password'
```

`npm run setup` applies `schema.sql`. You can also open `/setup` in the browser once, instead of `create-user`, while the users table is empty.

## 2. Run Next.js

cPanel → Setup Node.js App:

- Application root: the `server` directory
- Application startup file is not used; set the start command to `npm start`
- The selector provides `PORT`. The start script listens on `127.0.0.1` and that port.

Or, in a jailed shell:

```bash
npm run build
npm start
```

If the process is stopped when you log out, add a cron job every five minutes:

```bash
bash /home/USER/mindmap/server/scripts/ensure-running.sh
```

## 3. Put it on the public domain

**Node.js application (preferred when the cPanel Node selector is available).** Point the application URL at the domain or subdomain. The desktop cloud address is `https://that-domain`. No PHP is required.

**PHP proxy (when the domain's document root is Apache/nginx and Node stays on a local port).** Copy `cpanel/public/` to the domain document root (`public_html`, or the subdomain folder). Copy `cpanel/composer.json` to the parent of that folder if you want vendor outside the web root, or into the document root's parent as laid out here:

```text
cpanel/composer.json
cpanel/upstream.txt          optional, one line: http://127.0.0.1:3000
cpanel/vendor/               created by composer
cpanel/public/index.php      this directory is the document root
cpanel/public/.htaccess
```

```bash
cd cpanel
composer install --no-dev
```

`upstream.txt` overrides the default `http://127.0.0.1:3000` when cPanel will not pass `MINDMAP_UPSTREAM`. The desktop cloud address is the public domain, not the local port.

The WHM nginx layer normally proxies to Apache, and Apache reads `.htaccess`. `cpanel/nginx-snippet.conf` is only for a vhost include that should skip PHP and proxy straight to Next.js.

Use `https://` for the public address. The desktop program will accept `http://` but tells you the password would travel in the clear.

## Sync behaviour

The map document is the same JSON the desktop stores: `version`, `root_order`, `last_open`, and `canvases`. A push looks like:

```json
{
  "base_revision": 3,
  "client_id": "device-id",
  "force": false,
  "library": {}
}
```

`200` means the cloud stored it and bumped the revision, after saving the previous document as a backup. The same hash as the current document is accepted without a new revision. `409` means the cloud revision is not `base_revision`; the body includes the cloud `library`. `force: true` replaces the cloud anyway and still keeps a backup.

`POST /api/v1/backups` with `{ "label": "Before the trip" }` stores the current map. `POST /api/v1/backups/<id>/restore` puts that backup back and keeps the map it replaced.

Login is limited to 8 failures per username and 30 per address in 15 minutes. Passwords are stored as bcrypt hashes.
