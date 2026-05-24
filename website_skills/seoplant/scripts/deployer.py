"""
SEOplant Deployer — Phase 5 of the pipeline.

Generates deployment configurations, pushes to customer VPS via SSH,
bootstraps server environments, and supports one-click deploy.

Deployment targets:
  - vps       — SSH push to customer's own VPS (BYOV model)
  - package   — Generate config files for manual setup
  - vercel    — Deploy via Vercel CLI / API
  - cloudflare — Deploy via Wrangler to Cloudflare Pages

Dependencies: none required (paramiko optional, falls back to subprocess ssh)
"""

import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import time
import urllib.request
from datetime import datetime
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Optional SSH library — graceful fallback to subprocess
# ---------------------------------------------------------------------------
try:
    import paramiko
    HAS_PARAMIKO = True
except ImportError:
    HAS_PARAMIKO = False


# ===================================================================
# 1. Nginx Configuration
# ===================================================================

def generate_nginx_config(
    domain: str,
    root_path: str = "/www/wwwroot/{domain}/dist",
    ssl: bool = False,
    upstream_services: dict = None,
) -> str:
    """Generate Nginx configuration for Baota panel or manual setup."""
    upstream_services = upstream_services or {}
    root = root_path.replace("{domain}", domain)

    upstream_blocks = ""
    location_blocks = ""
    for service, port in upstream_services.items():
        upstream_blocks += f"""
upstream {service} {{
    server 127.0.0.1:{port};
}}
"""
        location_blocks += f"""
    # {service.title()} reverse proxy
    location /{service}/ {{
        proxy_pass http://{service}/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
"""

    listen_block = "listen 80;" if not ssl else f"""listen 80;
    listen 443 ssl http2;
    ssl_certificate /etc/letsencrypt/live/{domain}/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/{domain}/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # HTTP -> HTTPS redirect
    if ($server_port = 80) {{
        return 301 https://$host$request_uri;
    }}"""

    return f"""{upstream_blocks}
server {{
    {listen_block}
    server_name {domain} www.{domain};
    root {root};
    index index.html;

    # Gzip compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types
        text/plain
        text/css
        text/javascript
        application/javascript
        application/json
        application/xml
        image/svg+xml
        font/woff2;

    # Static asset caching (1 year for hashed assets)
    location ~* \\.(css|js|jpg|jpeg|png|gif|ico|svg|woff2|woff|ttf|eot)$ {{
        expires 1y;
        add_header Cache-Control "public, immutable";
        access_log off;
    }}

    # HTML pages (no cache for dynamic content)
    location ~* \\.html$ {{
        add_header Cache-Control "no-cache";
    }}
{location_blocks}
    # Astro static site routing
    location / {{
        try_files $uri $uri/ $uri.html /index.html =404;
    }}

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Permissions-Policy "camera=(), microphone=(), geolocation=()" always;

    # Block common attack patterns
    location ~* /(wp-admin|wp-login|xmlrpc\\.php|wp-includes) {{
        return 404;
    }}

    # Error pages
    error_page 404 /404.html;
    error_page 500 502 503 504 /50x.html;

    # Access and error logs
    access_log /var/log/nginx/{domain}.log;
    error_log /var/log/nginx/{domain}.error.log;
}}
"""


# ===================================================================
# 1b. Caddy Configuration (simpler alternative — auto SSL, fewer lines)
# ===================================================================

def generate_caddy_config(
    domain: str,
    root_path: str = "/www/wwwroot/{domain}/dist",
    upstream_services: dict = None,
) -> str:
    """Generate Caddyfile for the site.

    Caddy auto-handles SSL via Let's Encrypt — no certbot needed.
    Preferred for customer VPS deployments where simplicity matters.
    """
    upstream_services = upstream_services or {}
    root = root_path.replace("{domain}", domain)

    reverse_proxy_lines = ""
    for service, port in upstream_services.items():
        reverse_proxy_lines += f"""
    handle /{service}/* {{
        reverse_proxy localhost:{port}
    }}"""

    return f"""{domain}, www.{domain} {{

    root * {root}
    file_server
    encode gzip

    # Security headers
    header {{
        X-Frame-Options "SAMEORIGIN"
        X-Content-Type-Options "nosniff"
        X-XSS-Protection "1; mode=block"
        Referrer-Policy "strict-origin-when-cross-origin"
        Permissions-Policy "camera=(), microphone=(), geolocation=()"
    }}

    # Pretty URLs (no .html extension)
    try_files {{path}} {{path}}.html {{path}}/index.html

    # Cache static assets
    @static {{
        path *.css *.js *.jpg *.jpeg *.png *.gif *.ico *.svg *.woff2 *.woff *.ttf *.eot
    }}
    header @static {{
        Cache-Control "public, max-age=31536000, immutable"
    }}

    # Logs
    log {{
        output file /var/log/caddy/{domain}.log
    }}
{reverse_proxy_lines}
}}
"""


# ===================================================================
# 2. Docker Compose
# ===================================================================

def generate_docker_compose(
    domain: str,
    modules: dict = None,
) -> str:
    """Generate docker-compose.yml for optional services.

    Supports: Directus CMS, Medusa ecommerce, Plausible analytics, Tianji
    """
    modules = modules or {}
    services = {}
    volumes = {}

    if modules.get("cms"):
        services["directus"] = {
            "image": "directus/directus:latest",
            "restart": "unless-stopped",
            "ports": ["8055:8055"],
            "environment": {
                "SECRET": "change-this-to-a-random-secret-key",
                "ADMIN_EMAIL": "admin@example.com",
                "ADMIN_PASSWORD": "changeme-on-first-login",
                "DB_CLIENT": "sqlite3",
                "DB_FILENAME": "/directus/database/data.db",
                "CORS_ENABLED": "true",
                "CORS_ORIGIN": f"https://{domain}",
            },
            "volumes": [
                "directus-data:/directus/database",
                "directus-uploads:/directus/uploads",
            ],
        }
        volumes["directus-data"] = None
        volumes["directus-uploads"] = None

    if modules.get("ecommerce"):
        services["medusa"] = {
            "image": "medusajs/medusa:latest",
            "restart": "unless-stopped",
            "ports": ["9000:9000"],
            "environment": {
                "DATABASE_URL": "postgres://medusa:medusa@postgres:5432/medusa",
                "STORE_CORS": f"https://{domain}",
            },
            "depends_on": ["postgres"],
        }
        services["postgres"] = {
            "image": "postgres:16-alpine",
            "restart": "unless-stopped",
            "environment": {
                "POSTGRES_USER": "medusa",
                "POSTGRES_PASSWORD": "medusa",
                "POSTGRES_DB": "medusa",
            },
            "volumes": ["postgres-data:/var/lib/postgresql/data"],
        }
        volumes["postgres-data"] = None

    if modules.get("analytics"):
        analytics_tool = modules.get("analytics_tool", "plausible")
        if analytics_tool == "tianji":
            services["tianji"] = {
                "image": "moonrailgun/tianji:latest",
                "restart": "unless-stopped",
                "ports": ["12345:12345"],
                "volumes": ["tianji-data:/app/data"],
            }
            volumes["tianji-data"] = None
        else:
            services["plausible"] = {
                "image": "ghcr.io/plausible/community-edition:v2-latest",
                "restart": "unless-stopped",
                "ports": ["8000:8000"],
                "environment": {
                    "BASE_URL": f"https://analytics.{domain}",
                    "SECRET_KEY_BASE": "change-this-to-a-64-char-random-string",
                    "TOTP_VAULT_KEY": "change-this-to-another-random-key",
                },
                "volumes": ["plausible-data:/var/lib/plausible"],
            }
            volumes["plausible-data"] = None

    if not services:
        return "# No services configured. Add modules in project-brief.yaml."

    yaml_lines = ["version: '3.8'", "", "services:"]
    for name, config in services.items():
        yaml_lines.append(f"  {name}:")
        for key, value in config.items():
            if key == "environment" and isinstance(value, dict):
                yaml_lines.append("    environment:")
                for env_key, env_val in value.items():
                    yaml_lines.append(f'      {env_key}: "{env_val}"')
            elif key == "volumes" and isinstance(value, list):
                yaml_lines.append("    volumes:")
                for v in value:
                    yaml_lines.append(f"      - {v}")
            elif key == "ports" and isinstance(value, list):
                yaml_lines.append("    ports:")
                for p in value:
                    yaml_lines.append(f'      - "{p}"')
            elif key == "depends_on" and isinstance(value, list):
                yaml_lines.append("    depends_on:")
                for d in value:
                    yaml_lines.append(f"      - {d}")
            elif isinstance(value, str):
                yaml_lines.append(f"    {key}: {value}")

    if volumes:
        yaml_lines.append("")
        yaml_lines.append("volumes:")
        for vol_name in volumes:
            yaml_lines.append(f"  {vol_name}:")

    return "\n".join(yaml_lines)


# ===================================================================
# 3. Deployment Script (manual / CI mode)
# ===================================================================

def generate_deploy_script(
    domain: str,
    server_user: str = "root",
    server_host: str = "your-server-ip",
    deploy_path: str = "/www/wwwroot/{domain}",
    has_docker: bool = False,
) -> str:
    """Generate deploy.sh for manual or CI-based deployment."""
    path = deploy_path.replace("{domain}", domain)

    script = f"""#!/bin/bash
# Deployment script for {domain}
# Generated: {datetime.now().strftime('%Y-%m-%d')}

set -e

echo "Building site..."
npm run build

echo "Deploying to {server_host}..."
rsync -avz --delete \\
  --exclude '.git' \\
  --exclude 'node_modules' \\
  dist/ {server_user}@{server_host}:{path}/dist/

echo "Reloading Nginx..."
ssh {server_user}@{server_host} "nginx -t && nginx -s reload"
"""

    if has_docker:
        script += f"""
echo "Updating Docker services..."
scp docker-compose.yml {server_user}@{server_host}:{path}/
ssh {server_user}@{server_host} "cd {path} && docker compose pull && docker compose up -d"
"""

    script += """
echo "Deployment complete!"
echo "Visit: https://{domain}"
""".replace("{domain}", domain)

    return script


# ===================================================================
# 4. Analytics Setup
# ===================================================================

def generate_analytics_snippet(
    domain: str,
    tool: str = "plausible",
    analytics_url: str = None,
) -> str:
    """Generate analytics script tag for insertion in BaseLayout."""
    if tool == "plausible":
        src = analytics_url or f"https://analytics.{domain}"
        return f'<script defer data-domain="{domain}" src="{src}/js/script.js"></script>'
    elif tool == "tianji":
        src = analytics_url or f"https://tianji.{domain}"
        return f'<script async defer src="{src}/tracker.js" data-website-id="YOUR-WEBSITE-ID"></script>'
    elif tool == "umami":
        src = analytics_url or f"https://analytics.{domain}"
        return f'<script async defer src="{src}/umami.js" data-website-id="YOUR-WEBSITE-ID"></script>'
    else:
        return f"<!-- Add your analytics script here for {domain} -->"


# ===================================================================
# 5. Full Deployment Package (offline mode — generate files only)
# ===================================================================

def generate_deployment_package(
    project_dir: str,
    domain: str,
    modules: dict = None,
    server_user: str = "root",
    server_host: str = "your-server-ip",
    ssl: bool = True,
) -> dict:
    """Generate all deployment files in the deploy/ directory."""
    modules = modules or {}
    base = Path(project_dir) / "deploy"
    base.mkdir(parents=True, exist_ok=True)
    created = []

    upstreams = {}
    if modules.get("cms"):
        upstreams["directus"] = 8055
    if modules.get("ecommerce"):
        upstreams["medusa"] = 9000
    if modules.get("analytics"):
        tool = modules.get("analytics_tool", "plausible")
        upstreams["plausible" if tool == "plausible" else "tianji"] = (
            8000 if tool == "plausible" else 12345
        )

    # Nginx config
    nginx = generate_nginx_config(domain, ssl=ssl, upstream_services=upstreams)
    (base / f"{domain}.conf").write_text(nginx, encoding="utf-8")
    created.append(f"deploy/{domain}.conf")

    # Caddyfile (alternative)
    caddy = generate_caddy_config(domain, upstream_services=upstreams)
    (base / "Caddyfile").write_text(caddy, encoding="utf-8")
    created.append("deploy/Caddyfile")

    # Docker Compose
    if modules.get("cms") or modules.get("ecommerce") or modules.get("analytics"):
        compose = generate_docker_compose(domain, modules)
        (base / "docker-compose.yml").write_text(compose, encoding="utf-8")
        created.append("deploy/docker-compose.yml")

    # Deploy script
    has_docker = bool(modules.get("cms") or modules.get("ecommerce") or modules.get("analytics"))
    deploy_sh = generate_deploy_script(domain, server_user, server_host, has_docker=has_docker)
    deploy_path = base / "deploy.sh"
    deploy_path.write_text(deploy_sh, encoding="utf-8")
    created.append("deploy/deploy.sh")

    # Analytics snippet
    if modules.get("analytics"):
        tool = modules.get("analytics_tool", "plausible")
        snippet = generate_analytics_snippet(domain, tool)
        (base / "analytics-snippet.html").write_text(snippet, encoding="utf-8")
        created.append("deploy/analytics-snippet.html")

    # README
    readme = f"""# Deployment Guide for {domain}

## Option A: One-click VPS Deploy (recommended)
```bash
python deployer.py vps deploy ./project {domain} --host YOUR_VPS_IP --user root
```

## Option B: Manual Setup
1. Copy `{domain}.conf` to `/etc/nginx/sites-available/`
2. Enable: `ln -sf /etc/nginx/sites-available/{domain} /etc/nginx/sites-enabled/`
3. Run `nginx -t && nginx -s reload`
4. For SSL: `certbot --nginx -d {domain} -d www.{domain}`
5. If using Docker: `docker compose up -d`

## Option C: Caddy (simpler — auto SSL)
1. Copy `Caddyfile` to `/etc/caddy/`
2. Run `caddy reload`

Generated: {datetime.now().strftime('%Y-%m-%d %H:%M')}
"""
    (base / "README.md").write_text(readme, encoding="utf-8")
    created.append("deploy/README.md")

    return {"created_files": created}


# ===================================================================
# 6. VPSDeployer — SSH-based one-click deployment
# ===================================================================

# Known package managers and their install commands
_PKG_MANAGERS = {
    "apt": {
        "update": "apt-get update -qq",
        "install": "apt-get install -y -qq {packages}",
        "check": "dpkg -s {package} 2>/dev/null | grep -q 'ok installed'",
    },
    "yum": {
        "update": "yum check-update -q || true",
        "install": "yum install -y -q {packages}",
        "check": "rpm -q {package} >/dev/null 2>&1",
    },
    "dnf": {
        "update": "dnf check-update -q || true",
        "install": "dnf install -y -q {packages}",
        "check": "rpm -q {package} >/dev/null 2>&1",
    },
    "apk": {
        "update": "apk update -q",
        "install": "apk add --no-cache {packages}",
        "check": "apk info -e {package} >/dev/null 2>&1",
    },
    "zypper": {
        "update": "zypper refresh -q",
        "install": "zypper install -y {packages}",
        "check": "rpm -q {package} >/dev/null 2>&1",
    },
}

# Required system packages for each web server
_REQUIRED_PACKAGES = {
    "nginx": {
        "apt": ["nginx", "certbot", "python3-certbot-nginx"],
        "yum": ["nginx", "certbot", "python3-certbot-nginx"],
        "dnf": ["nginx", "certbot", "python3-certbot-nginx"],
        "apk": ["nginx", "certbot", "certbot-nginx"],
        "zypper": ["nginx", "certbot", "python3-certbot-nginx"],
    },
    "caddy": {
        "apt": ["caddy"],  # Caddy auto-SSL, no certbot needed
        "yum": ["caddy"],
        "dnf": ["caddy"],
        "apk": ["caddy"],
        "zypper": ["caddy"],
    },
}


class VPSDeployer:
    """SSH-based one-click deployment to a customer's own VPS.

    The "Bring Your Own VPS" model — no platform lock-in.

    Usage:
        deployer = VPSDeployer("12.34.56.78", user="root")
        deployer.check_environment()
        deployer.bootstrap(web_server="caddy")
        deployer.deploy("mysite.com", "./project", modules={"analytics": True})
    """

    def __init__(
        self,
        host: str,
        user: str = "root",
        port: int = 22,
        key_path: str = None,
        password: str = None,
        on_progress = None,
    ):
        self.host = host
        self.user = user
        self.port = port
        self.key_path = key_path or os.path.expanduser("~/.ssh/id_rsa")
        self.password = password
        self._on_progress = on_progress or (lambda msg: print(f"  {msg}"))
        self._client = None
        self._sftp = None
        self._server_info = None  # Cached environment info

    # ------- connection management -------

    def _connect(self):
        """Establish SSH connection (idempotent)."""
        if self._client is not None:
            return

        if HAS_PARAMIKO:
            self._client = paramiko.SSHClient()
            self._client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            try:
                if self.password:
                    self._client.connect(
                        self.host, self.port, self.user, self.password,
                        timeout=15, allow_agent=False, look_for_keys=False,
                    )
                else:
                    self._client.connect(
                        self.host, self.port, self.user,
                        key_filename=self.key_path, timeout=15,
                    )
            except paramiko.AuthenticationException:
                raise ConnectionError(
                    f"SSH auth failed for {self.user}@{self.host}. "
                    f"Check your key or password."
                )
            except Exception as e:
                raise ConnectionError(f"SSH connection failed: {e}")
        else:
            # Subprocess fallback — check connectivity only
            self._on_progress(
                "[WARN] paramiko not installed. Using subprocess ssh. "
                "Install paramiko for richer error handling: pip install paramiko"
            )

    def _close(self):
        if self._sftp:
            self._sftp.close()
            self._sftp = None
        if self._client:
            self._client.close()
            self._client = None

    def _run(self, command: str, sudo: bool = False) -> tuple[int, str, str]:
        """Run a command on the remote host. Returns (exit_code, stdout, stderr)."""
        if sudo and self.user != "root":
            command = f"sudo {command}"

        if HAS_PARAMIKO and self._client:
            self._connect()
            stdin, stdout, stderr = self._client.exec_command(command)
            exit_code = stdout.channel.recv_exit_status()
            return exit_code, stdout.read().decode("utf-8", errors="replace"), stderr.read().decode("utf-8", errors="replace")
        else:
            ssh_cmd = [
                "ssh",
                "-o", "StrictHostKeyChecking=accept-new",
                "-o", "ConnectTimeout=15",
                "-p", str(self.port),
            ]
            if self.key_path:
                ssh_cmd += ["-i", self.key_path]
            ssh_cmd.append(f"{self.user}@{self.host}")
            ssh_cmd.append(command)

            result = subprocess.run(ssh_cmd, capture_output=True, text=True, timeout=60)
            return result.returncode, result.stdout, result.stderr

    def _put_file(self, local_path: str, remote_path: str):
        """Upload a single file to the remote host."""
        if HAS_PARAMIKO and self._client:
            self._connect()
            if self._sftp is None:
                self._sftp = self._client.open_sftp()
            self._sftp.put(local_path, remote_path)
        else:
            subprocess.run(
                [
                    "scp", "-o", "StrictHostKeyChecking=accept-new",
                    "-P", str(self.port),
                    "-i", self.key_path,
                    local_path,
                    f"{self.user}@{self.host}:{remote_path}",
                ],
                check=True, capture_output=True, timeout=60,
            )

    def _put_directory(self, local_dir: str, remote_dir: str):
        """Upload a directory recursively."""
        local = Path(local_dir)
        if not local.is_dir():
            raise ValueError(f"Not a directory: {local_dir}")

        if HAS_PARAMIKO and self._client:
            self._connect()
            if self._sftp is None:
                self._sftp = self._client.open_sftp()

            # Ensure remote dir exists
            self._run(f"mkdir -p {remote_dir}")
            for file_path in local.rglob("*"):
                if file_path.is_file():
                    rel = file_path.relative_to(local)
                    remote_file = f"{remote_dir}/{rel}".replace("\\", "/")
                    remote_parent = str(Path(remote_file).parent).replace("\\", "/")
                    try:
                        self._sftp.mkdir(remote_parent)
                    except IOError:
                        pass  # Directory already exists
                    self._sftp.put(str(file_path), remote_file)
        else:
            # rsync is much faster for directories
            subprocess.run(
                [
                    "rsync", "-avz", "--delete",
                    "-e", f"ssh -o StrictHostKeyChecking=accept-new -p {self.port} -i {self.key_path}",
                    f"{local_dir}/",
                    f"{self.user}@{self.host}:{remote_dir}/",
                ],
                check=True, capture_output=True, timeout=300,
            )

    # ------- server introspection -------

    def check_environment(self) -> dict:
        """Pre-flight check: OS, packages, disk, memory, open ports.

        Returns a dict with server_info suitable for display and decision-making.
        """
        self._connect()
        self._on_progress(f"Checking environment on {self.host}...")

        exit_code, stdout, stderr = self._run(
            "echo '---OS---'; cat /etc/os-release 2>/dev/null | head -5; "
            "echo '---PKG---'; (which apt-get || which yum || which dnf || "
            "which apk || which zypper || echo 'unknown') 2>/dev/null; "
            "echo '---DISK---'; df -h / | tail -1; "
            "echo '---MEM---'; free -m | grep Mem; "
            "echo '---NGINX---'; which nginx 2>/dev/null || echo 'not-found'; "
            "echo '---CADDY---'; which caddy 2>/dev/null || echo 'not-found'; "
            "echo '---DOCKER---'; which docker 2>/dev/null || echo 'not-found'; "
            "echo '---CERTBOT---'; which certbot 2>/dev/null || echo 'not-found'; "
            "echo '---PORT80---'; ss -tlnp 2>/dev/null | grep ':80 ' || echo 'free'; "
            "echo '---PORT443---'; ss -tlnp 2>/dev/null | grep ':443 ' || echo 'free'; "
        )

        info = {"host": self.host, "user": self.user, "raw": stdout}

        # Parse OS
        os_id = re.search(r'^ID="?([^"\n]+)"?', stdout, re.M)
        info["os"] = os_id.group(1) if os_id else "unknown"

        # Parse package manager
        pkg = re.search(r'/(apt-get|yum|dnf|apk|zypper)', stdout)
        info["pkg_manager"] = pkg.group(1) if pkg else None

        # Parse disk
        disk = re.search(r'(\d+)%\s+/$', stdout, re.M)
        info["disk_used_pct"] = int(disk.group(1)) if disk else None

        # Parse memory
        mem = re.search(r'Mem:\s+\d+\s+(\d+)', stdout)
        info["mem_used_mb"] = int(mem.group(1)) if mem else None

        # Installed tools
        info["has_nginx"] = "not-found" not in self._extract(stdout, "NGINX")
        info["has_caddy"] = "not-found" not in self._extract(stdout, "CADDY")
        info["has_docker"] = "not-found" not in self._extract(stdout, "DOCKER")
        info["has_certbot"] = "not-found" not in self._extract(stdout, "CERTBOT")
        info["port80_free"] = "free" in self._extract(stdout, "PORT80")
        info["port443_free"] = "free" in self._extract(stdout, "PORT443")

        self._server_info = info
        return info

    def _extract(self, text: str, marker: str) -> str:
        """Extract the line after a marker in a multi-section output."""
        pattern = rf'---{marker}---\n(.+?)(?:\n---|$)'
        match = re.search(pattern, text)
        return match.group(1).strip() if match else ""

    # ------- server bootstrap -------

    def bootstrap(
        self,
        web_server: str = "caddy",
        with_docker: bool = False,
        dry_run: bool = False,
    ) -> dict:
        """Initialize a fresh VPS with the required stack.

        Args:
            web_server: 'nginx', 'caddy', or 'none'
            with_docker: Install Docker CE as well
            dry_run: Print what would be done without doing it

        Returns:
            Dict with steps taken and their results
        """
        if not self._server_info:
            self.check_environment()

        info = self._server_info
        pkg = info.get("pkg_manager")
        if not pkg:
            return {"error": "Unsupported OS — no apt/yum/dnf/apk detected."}

        pkgs = _REQUIRED_PACKAGES.get(web_server, {}).get(pkg, [])
        steps = []
        pkg_cfg = _PKG_MANAGERS.get(pkg, {})

        # Step 1: Update package index
        if pkg_cfg.get("update"):
            cmd = pkg_cfg["update"]
            if dry_run:
                steps.append({"step": "update_packages", "cmd": cmd, "status": "dry_run"})
            else:
                self._on_progress("Updating package index...")
                ec, out, err = self._run(cmd, sudo=True)
                steps.append({"step": "update_packages", "cmd": cmd, "exit": ec, "status": "ok" if ec == 0 else "failed"})

        # Step 2: Install web server and SSL tools
        if pkgs:
            cmd = pkg_cfg["install"].format(packages=" ".join(pkgs))
            if dry_run:
                steps.append({"step": f"install_{web_server}", "cmd": cmd, "packages": pkgs, "status": "dry_run"})
            else:
                self._on_progress(f"Installing {web_server} + SSL tools...")
                ec, out, err = self._run(cmd, sudo=True)
                steps.append({
                    "step": f"install_{web_server}",
                    "cmd": cmd,
                    "exit": ec,
                    "status": "ok" if ec == 0 else "failed",
                    "stderr": err[:500] if ec != 0 else None,
                })

        # Step 3: Enable and start web server
        if web_server != "none":
            for svc in [web_server]:
                cmd = f"systemctl enable {svc} && systemctl start {svc}"
                if dry_run:
                    steps.append({"step": f"enable_{svc}", "status": "dry_run"})
                else:
                    ec, out, err = self._run(cmd, sudo=True)
                    steps.append({"step": f"enable_{svc}", "status": "ok" if ec == 0 else "failed"})

        # Step 4: Create web root
        cmd = "mkdir -p /www/wwwroot /www/wwwlogs"
        if dry_run:
            steps.append({"step": "create_web_root", "status": "dry_run"})
        else:
            self._run(cmd, sudo=True)
            steps.append({"step": "create_web_root", "status": "ok"})

        # Step 5: Docker (optional)
        if with_docker and not info.get("has_docker"):
            cmd = "curl -fsSL https://get.docker.com | sh"
            if dry_run:
                steps.append({"step": "install_docker", "status": "dry_run"})
            else:
                self._on_progress("Installing Docker...")
                ec, out, err = self._run(cmd, sudo=True)
                steps.append({"step": "install_docker", "status": "ok" if ec == 0 else "failed"})
                if ec == 0:
                    self._run(f"usermod -aG docker {self.user}", sudo=True)

        self._on_progress(f"Bootstrap complete: {len([s for s in steps if s.get('status') == 'ok'])} steps OK")
        return {"server": f"{self.user}@{self.host}", "web_server": web_server, "steps": steps}

    # ------- deploy static site -------

    def deploy(
        self,
        domain: str,
        project_dir: str,
        modules: dict = None,
        web_server: str = "caddy",
        ssl: bool = True,
        dry_run: bool = False,
    ) -> dict:
        """Deploy a complete Astro static site to the VPS.

        This is the main entry point — it does everything:
        1. Build the site (npm run build)
        2. Upload static files
        3. Configure web server
        4. Obtain SSL certificate
        5. Verify deployment

        Args:
            domain: The website domain name
            project_dir: Path to the Astro project directory
            modules: Optional services dict (cms, analytics, ecommerce)
            web_server: 'nginx' or 'caddy'
            ssl: Whether to enable HTTPS
            dry_run: Print actions without executing

        Returns:
            Dict with deployment result and verification URL
        """
        modules = modules or {}
        project = Path(project_dir)
        actions = []

        # Build upstream map
        upstreams = {}
        has_docker = False
        if modules.get("cms"):
            upstreams["directus"] = 8055
            has_docker = True
        if modules.get("ecommerce"):
            upstreams["medusa"] = 9000
            has_docker = True
        if modules.get("analytics"):
            tool = modules.get("analytics_tool", "plausible")
            upstreams["plausible" if tool == "plausible" else "tianji"] = (
                8000 if tool == "plausible" else 12345
            )
            has_docker = True

        deploy_root = f"/www/wwwroot/{domain}"
        dist_dir = f"{deploy_root}/dist"

        # Step 1: Build the Astro project
        dist_path = project / "dist"
        if dist_path.exists():
            self._on_progress(f"Using existing build at {dist_path}")
            actions.append({"step": "build", "status": "skipped", "reason": "dist/ already exists"})
        else:
            self._on_progress("Building Astro project...")
            if dry_run:
                actions.append({"step": "build", "cmd": "npm run build", "status": "dry_run"})
            else:
                result = subprocess.run(
                    ["npm", "run", "build"],
                    cwd=str(project),
                    capture_output=True, text=True, timeout=120,
                )
                if result.returncode != 0:
                    return {"error": "Build failed", "stderr": result.stderr[-1000:]}
                actions.append({"step": "build", "status": "ok"})

        # Step 2: Create remote directories
        if not dry_run:
            self._run(f"mkdir -p {dist_dir} /var/log/nginx 2>/dev/null; "
                      f"mkdir -p /var/log/caddy 2>/dev/null; "
                      f"chown -R $USER:$USER /www/wwwroot 2>/dev/null || true",
                      sudo=True)
        actions.append({"step": "create_dirs", "path": deploy_root, "status": "dry_run" if dry_run else "ok"})

        # Step 3: Upload static files
        self._on_progress(f"Uploading static files to {self.host}:{dist_dir}/...")
        if dry_run:
            actions.append({"step": "upload_files", "from": str(dist_path), "to": f"{self.user}@{self.host}:{dist_dir}/", "status": "dry_run"})
        else:
            self._put_directory(str(dist_path), dist_dir)
            actions.append({"step": "upload_files", "files": "dist/*", "status": "ok"})

        # Step 4: Configure web server
        self._on_progress(f"Configuring {web_server} for {domain}...")
        if web_server == "caddy":
            caddy_conf = generate_caddy_config(domain, upstream_services=upstreams)
            if dry_run:
                actions.append({"step": f"configure_{web_server}", "status": "dry_run"})
            else:
                # Write Caddyfile snippet to /etc/caddy/sites/
                self._run("mkdir -p /etc/caddy/sites", sudo=True)
                tmp = tempfile.NamedTemporaryFile(mode="w", suffix=".caddy", delete=False, encoding="utf-8")
                tmp.write(caddy_conf)
                tmp.close()
                self._put_file(tmp.name, f"/etc/caddy/sites/{domain}.caddy")
                os.unlink(tmp.name)

                # Include sites directory in main Caddyfile if not already
                ec, out, _ = self._run("grep -q 'import /etc/caddy/sites/' /etc/caddy/Caddyfile")
                if ec != 0:
                    self._run(
                        "sed -i '1i import /etc/caddy/sites/*.caddy' /etc/caddy/Caddyfile",
                        sudo=True,
                    )

                self._run(f"caddy fmt --overwrite /etc/caddy/sites/{domain}.caddy || true", sudo=True)
                self._run("caddy reload", sudo=True)
                actions.append({"step": f"configure_{web_server}", "status": "ok"})
        else:
            nginx_conf = generate_nginx_config(domain, ssl=ssl, upstream_services=upstreams)
            if dry_run:
                actions.append({"step": "configure_nginx", "status": "dry_run"})
            else:
                tmp = tempfile.NamedTemporaryFile(mode="w", suffix=".conf", delete=False, encoding="utf-8")
                tmp.write(nginx_conf)
                tmp.close()
                self._put_file(tmp.name, f"/etc/nginx/sites-available/{domain}")
                os.unlink(tmp.name)
                self._run(
                    f"ln -sf /etc/nginx/sites-available/{domain} /etc/nginx/sites-enabled/{domain}",
                    sudo=True,
                )
                self._run("nginx -t", sudo=True)
                self._run("nginx -s reload", sudo=True)
                actions.append({"step": "configure_nginx", "status": "ok"})

        # Step 5: SSL certificate (Nginx only — Caddy is automatic)
        if ssl and web_server == "nginx":
            self._on_progress("Obtaining SSL certificate...")
            if dry_run:
                actions.append({"step": "ssl_cert", "status": "dry_run"})
            else:
                ec, out, err = self._run(
                    f"certbot --nginx -d {domain} -d www.{domain} "
                    "--non-interactive --agree-tos --email admin@{domain} "
                    "--redirect 2>&1 || echo 'certbot_failed'",
                    sudo=True,
                )
                if "certbot_failed" in out or ec != 0:
                    self._on_progress(f"[WARN] SSL failed — running HTTP-only. {err[:200]}")
                    actions.append({"step": "ssl_cert", "status": "failed", "hint": "Check that DNS points to this server"})
                else:
                    actions.append({"step": "ssl_cert", "status": "ok"})

        # Step 6: Docker services
        if has_docker:
            compose_yaml = generate_docker_compose(domain, modules)
            if dry_run:
                actions.append({"step": "docker_compose", "status": "dry_run"})
            else:
                tmp = tempfile.NamedTemporaryFile(mode="w", suffix=".yml", delete=False, encoding="utf-8")
                tmp.write(compose_yaml)
                tmp.close()
                self._put_file(tmp.name, f"{deploy_root}/docker-compose.yml")
                os.unlink(tmp.name)
                self._run(f"cd {deploy_root} && docker compose pull && docker compose up -d")
                actions.append({"step": "docker_compose", "status": "ok"})

        # Step 7: Verify
        verified = False
        if not dry_run:
            self._on_progress(f"Verifying deployment at https://{domain}...")
            try:
                import urllib.request
                req = urllib.request.Request(
                    f"http{'s' if ssl else ''}://{domain}",
                    headers={"User-Agent": "SEOplant-Deployer/1.0"},
                )
                resp = urllib.request.urlopen(req, timeout=15)
                if resp.status in (200, 301, 302, 403):
                    verified = True
                    self._on_progress(f"Site is live! HTTP {resp.status}")
            except Exception as e:
                self._on_progress(f"[WARN] Verification request failed: {e}")
                self._on_progress("DNS may not have propagated yet — check back in a few minutes.")

        return {
            "domain": domain,
            "url": f"http{'s' if ssl else ''}://{domain}",
            "server": f"{self.user}@{self.host}",
            "deploy_path": deploy_root,
            "web_server": web_server,
            "verified": verified,
            "actions": actions,
        }

    # ------- generate agent install script -------

    def generate_agent_install_script(self, agent_version: str = "v0.1.0") -> str:
        """Generate the one-liner agent install script for this VPS.

        This is the script customers run once to bind their VPS
        to the SEOplant platform. After this, all deployments happen
        via the platform dashboard — no more SSH needed.
        """
        return f"""#!/bin/bash
# SEOplant Deploy Agent Installer
# Run this once on your VPS to enable one-click deployments.
# curl -fsSL https://seoplant.io/agent/install.sh | bash

set -e

echo "SEOplant Agent Installer {agent_version}"
echo "----------------------------------------"

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
else
    echo "Unsupported OS. Requires Ubuntu, Debian, CentOS, or Alpine."
    exit 1
fi

# Install dependencies
case $OS in
    ubuntu|debian)
        apt-get update -qq
        apt-get install -y -qq curl ca-certificates
        ;;
    centos|rhel|fedora)
        yum install -y -q curl ca-certificates || dnf install -y -q curl ca-certificates
        ;;
    alpine)
        apk add --no-cache curl ca-certificates
        ;;
    *)
        echo "Unknown OS: $OS. Continuing anyway..."
        ;;
esac

# Download agent binary
ARCH=$(uname -m)
case $ARCH in
    x86_64)  BIN_ARCH="amd64" ;;
    aarch64) BIN_ARCH="arm64" ;;
    *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

AGENT_URL="https://seoplant.io/agent/{agent_version}/seoplant-agent-linux-$BIN_ARCH"
echo "Downloading agent for $BIN_ARCH..."
curl -fsSL "$AGENT_URL" -o /usr/local/bin/seoplant-agent
chmod +x /usr/local/bin/seoplant-agent

# Create systemd service
cat > /etc/systemd/system/seoplant-agent.service <<'SVC'
[Unit]
Description=SEOplant Deploy Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/seoplant-agent
Restart=always
RestartSec=10
User=root
WorkingDirectory=/www

[Install]
WantedBy=multi-user.target
SVC

systemctl daemon-reload
systemctl enable seoplant-agent
systemctl start seoplant-agent

echo ""
echo "Agent installed successfully!"
echo ""
echo "Your Agent Key:"
cat /var/lib/seoplant/agent.key 2>/dev/null || \\
  /usr/local/bin/seoplant-agent --show-key 2>/dev/null || \\
  echo "(Check /var/lib/seoplant/agent.key)"
echo ""
echo "Copy this key to your SEOplant dashboard to complete setup."
echo "The agent is now running and listening for deployments."
"""


# ===================================================================
# 7. Vercel / Cloudflare / Netlify deploy helpers
# ===================================================================

def deploy_to_vercel(
    project_dir: str,
    domain: str = None,
    token: str = None,
    prod: bool = True,
) -> dict:
    """Deploy to Vercel via CLI. Requires `vercel` CLI installed."""
    # Check for vercel CLI
    if not shutil.which("vercel"):
        return {"error": "Vercel CLI not found. Install: npm i -g vercel"}

    cmd = ["vercel", str(project_dir), "--confirm"]
    if prod:
        cmd.append("--prod")
    if token:
        cmd.extend(["--token", token])

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    if result.returncode != 0:
        return {"error": "Vercel deploy failed", "stderr": result.stderr[-500:]}

    # Extract deployment URL from output
    url_match = re.search(r'https?://[\w.-]+\.vercel\.app', result.stdout)
    return {
        "status": "ok",
        "url": url_match.group(0) if url_match else None,
        "platform": "vercel",
        "stdout": result.stdout[-300:],
    }


def deploy_to_cloudflare(
    project_dir: str,
    domain: str = None,
    token: str = None,
    account_id: str = None,
) -> dict:
    """Deploy to Cloudflare Pages via Wrangler CLI."""
    if not shutil.which("wrangler"):
        return {"error": "Wrangler CLI not found. Install: npm i -g wrangler"}

    cmd = ["wrangler", "pages", "deploy", str(project_dir)]
    if domain:
        cmd.extend(["--domain", domain])

    env = os.environ.copy()
    if token:
        env["CLOUDFLARE_API_TOKEN"] = token
    if account_id:
        env["CLOUDFLARE_ACCOUNT_ID"] = account_id

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=120, env=env)
    if result.returncode != 0:
        return {"error": "Cloudflare deploy failed", "stderr": result.stderr[-500:]}

    url_match = re.search(r'https?://[\w.-]+\.pages\.dev', result.stdout)
    return {
        "status": "ok",
        "url": url_match.group(0) if url_match else None,
        "platform": "cloudflare",
        "stdout": result.stdout[-300:],
    }


# ===================================================================
# 7b. Backend Deploy — FastAPI to VPS with Caddy + systemd
# ===================================================================

def deploy_backend(
    backend_dir: str,
    domain: str,
    host: str,
    user: str = "root",
    port: int = 22,
    key_path: str = None,
    password: str = None,
    python_bin: str = "python3",
    dataforseo_email: str = "",
    dataforseo_password: str = "",
    secret_key: str = "",
    dry_run: bool = False,
) -> dict:
    """Deploy the FastAPI backend to a VPS with Caddy reverse proxy + systemd.

    Sets up:
      - /www/seoplant-backend/ — application code
      - /etc/systemd/system/seoplant-api.service — auto-start service
      - /etc/caddy/sites/api.{domain}.caddy — reverse proxy + auto SSL
    """
    deployer = VPSDeployer(host, user, port, key_path, password)
    actions = []

    app_dir = "/www/seoplant-backend"
    api_domain = f"api.{domain}"
    backend_src = Path(backend_dir)

    if not (backend_src / "main.py").exists():
        return {"error": f"backend/main.py not found in {backend_dir}"}

    # Step 1: Bootstrap — ensure Python + Caddy
    deployer._on_progress("Checking server environment...")
    info = deployer.check_environment()

    if not info.get("has_caddy"):
        deployer._on_progress("Installing Caddy...")
        if not dry_run:
            deployer.bootstrap(web_server="caddy")
        actions.append({"step": "install_caddy", "status": "dry_run" if dry_run else "ok"})

    # Step 2: Upload backend code
    deployer._on_progress(f"Uploading backend to {host}:{app_dir}/...")
    if dry_run:
        actions.append({"step": "upload_backend", "from": str(backend_src), "to": f"{user}@{host}:{app_dir}/", "status": "dry_run"})
    else:
        deployer._run(f"mkdir -p {app_dir}", sudo=True)
        deployer._put_directory(str(backend_src), app_dir)
        actions.append({"step": "upload_backend", "status": "ok"})

    # Step 3: Install Python dependencies
    deployer._on_progress("Installing Python dependencies...")
    if dry_run:
        actions.append({"step": "pip_install", "status": "dry_run"})
    else:
        ec, out, err = deployer._run(
            f"{python_bin} -m pip install -r {app_dir}/requirements.txt -q",
            sudo=True,
        )
        actions.append({"step": "pip_install", "status": "ok" if ec == 0 else "failed", "stderr": err[:300] if ec != 0 else ""})

    # Step 4: Create .env file
    env_content = f"""SEOPLANT_HOST=127.0.0.1
SEOPLANT_PORT=8800
SEOPLANT_DEBUG=false
SEOPLANT_SECRET={secret_key}
DATAFORSEO_EMAIL={dataforseo_email}
DATAFORSEO_PASSWORD={dataforseo_password}
"""
    if dry_run:
        actions.append({"step": "create_env", "status": "dry_run"})
    else:
        deployer._run(f"cat > {app_dir}/.env <<'ENVEOF'\n{env_content}\nENVEOF", sudo=True)
        deployer._run(f"chmod 600 {app_dir}/.env", sudo=True)
        actions.append({"step": "create_env", "status": "ok"})

    # Step 5: Create systemd service
    service_unit = f"""[Unit]
Description=SEOplant API Backend
After=network.target

[Service]
Type=simple
User=nobody
WorkingDirectory={app_dir}
EnvironmentFile={app_dir}/.env
ExecStart={python_bin} -m backend.main
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"""
    if dry_run:
        actions.append({"step": "create_systemd", "status": "dry_run"})
    else:
        deployer._run(
            f"cat > /etc/systemd/system/seoplant-api.service <<'SVC'\n{service_unit}\nSVC",
            sudo=True,
        )
        deployer._run("systemctl daemon-reload", sudo=True)
        deployer._run("systemctl enable seoplant-api", sudo=True)
        deployer._run("systemctl restart seoplant-api", sudo=True)
        actions.append({"step": "create_systemd", "status": "ok"})

    # Step 6: Caddy reverse proxy
    caddy_conf = f"""{api_domain} {{
    reverse_proxy 127.0.0.1:8800
    header {{
        X-Frame-Options "SAMEORIGIN"
        X-Content-Type-Options "nosniff"
        Referrer-Policy "strict-origin-when-cross-origin"
    }}
}}
"""
    if dry_run:
        actions.append({"step": "configure_caddy", "status": "dry_run"})
    else:
        deployer._run("mkdir -p /etc/caddy/sites", sudo=True)
        deployer._run(
            f"cat > /etc/caddy/sites/{api_domain}.caddy <<'CADDY'\n{caddy_conf}\nCADDY",
            sudo=True,
        )
        # Include sites dir if not already
        ec, _, _ = deployer._run("grep -q 'import /etc/caddy/sites/' /etc/caddy/Caddyfile")
        if ec != 0:
            deployer._run(
                "sed -i '1i import /etc/caddy/sites/*.caddy' /etc/caddy/Caddyfile",
                sudo=True,
            )
        deployer._run("caddy fmt --overwrite /etc/caddy/Caddyfile || true", sudo=True)
        deployer._run("caddy reload", sudo=True)
        actions.append({"step": "configure_caddy", "status": "ok"})

    # Step 7: Wait and verify
    if not dry_run:
        deployer._on_progress("Waiting for API to start...")
        import time
        time.sleep(3)
        deployer._on_progress(f"Verifying https://{api_domain}/api/auth/me ...")
        try:
            import urllib.request
            req = urllib.request.Request(
                f"https://{api_domain}/api/auth/me",
                headers={"User-Agent": "SEOplant-Deployer/1.0"},
            )
            resp = urllib.request.urlopen(req, timeout=20)
            if resp.status in (200, 403):  # 403 = working, just not authenticated
                actions.append({"step": "verify", "status": "ok", "http": resp.status})
            else:
                actions.append({"step": "verify", "status": "unexpected", "http": resp.status})
        except Exception as e:
            actions.append({"step": "verify", "status": "failed", "error": str(e)[:200]})

    return {
        "api_url": f"https://{api_domain}",
        "dashboard_url": f"https://{api_domain}/dashboard",
        "server": f"{user}@{host}",
        "app_dir": app_dir,
        "actions": actions,
    }


# ===================================================================
# 8. Environment check formatter
# ===================================================================

def format_env_report(info: dict) -> str:
    """Format a check_environment() result for display."""
    ok = "OK"
    warn = "WARN"
    missing = "MISS"

    lines = [
        f"Server: {info['host']} ({info.get('os', 'unknown')})",
        f"Package Manager: {info.get('pkg_manager') or missing}",
        f"Disk: {info.get('disk_used_pct', '?')}% used",
        f"Memory Used: {info.get('mem_used_mb', '?')} MB",
        "",
        "Installed:",
        f"  nginx:   {ok if info.get('has_nginx') else missing}",
        f"  caddy:   {ok if info.get('has_caddy') else missing}",
        f"  docker:  {ok if info.get('has_docker') else missing}",
        f"  certbot: {ok if info.get('has_certbot') else missing}",
        "",
        "Ports:",
        f"  :80  — {'free' if info.get('port80_free') else 'in use'}{warn if not info.get('port80_free') else ''}",
        f"  :443 — {'free' if info.get('port443_free') else 'in use'}{warn if not info.get('port443_free') else ''}",
    ]
    return "\n".join(lines)


# ===================================================================
# CLI Interface
# ===================================================================

def _parse_modules(argv: list[str]) -> dict:
    """Parse module flags from CLI args."""
    modules = {}
    tool = "plausible"
    for i, arg in enumerate(argv):
        if arg == "--analytics" and i + 1 < len(argv) and not argv[i + 1].startswith("-"):
            tool = argv[i + 1]
    return {
        "cms": "--cms" in argv,
        "analytics": "--analytics" in argv,
        "ecommerce": "--ecommerce" in argv,
        "analytics_tool": tool,
    }


def _parse_vps_args(argv: list[str]) -> dict:
    """Parse VPS connection args from CLI."""
    args = {"host": None, "user": "root", "port": 22, "key": None, "password": None}
    for i, arg in enumerate(argv):
        if arg == "--host" and i + 1 < len(argv):
            args["host"] = argv[i + 1]
        elif arg == "--user" and i + 1 < len(argv):
            args["user"] = argv[i + 1]
        elif arg == "--port" and i + 1 < len(argv):
            args["port"] = int(argv[i + 1])
        elif arg == "--key" and i + 1 < len(argv):
            args["key"] = argv[i + 1]
        elif arg == "--password" and i + 1 < len(argv):
            args["password"] = argv[i + 1]
        elif arg == "--password" and i + 1 < len(argv):
            args["password"] = argv[i + 1]
    return args


def main():
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2:
        print("""
SEOplant Deployer — multi-target deployment for Astro static sites.

Commands:
  check          Check a VPS environment before deployment
  bootstrap      Install web server + SSL tools on a fresh VPS
  deploy         One-click deploy Astro static site to VPS
  deploy-backend Deploy FastAPI backend to VPS with systemd + Caddy
  package        Generate deployment config files (manual setup)
  vercel         Deploy to Vercel
  cloudflare     Deploy to Cloudflare Pages
  nginx      Print Nginx configuration
  caddy      Print Caddy configuration
  docker     Print Docker Compose configuration

Usage:
  python deployer.py check --host 12.34.56.78 [--user root] [--key ~/.ssh/id_rsa]
  python deployer.py bootstrap --host 12.34.56.78 [--caddy|--nginx] [--docker]
  python deployer.py deploy ./project example.com \\
      --host 12.34.56.78 --user root [--caddy|--nginx] [--cms] [--analytics]
  python deployer.py deploy-backend ./backend example.com \\
      --host 12.34.56.78 [--dfseo-email hi@site.io] [--dfseo-password xxx]
  python deployer.py package ./project example.com [--cms] [--analytics]
  python deployer.py vercel ./project [--token xxx]
  python deployer.py nginx example.com [--ssl] [--cms]
  python deployer.py caddy example.com [--analytics]
  python deployer.py docker example.com [--cms] [--analytics]

Examples:
  # First time: bootstrap a fresh Ubuntu VPS with Caddy
  python deployer.py bootstrap --host 12.34.56.78 --caddy --docker

  # Deploy a static Astro site
  python deployer.py deploy ./scottish-highlands-travel scottish-highlands.com \\
      --host 12.34.56.78 --caddy --analytics

  # Deploy the backend API to a VPS
  python deployer.py deploy-backend ./backend seoplant.io \\
      --host 12.34.56.78 --dfseo-email hi@seoplant.io --dfseo-password ba6caffc26729445

  # Check what's on a VPS
  python deployer.py check --host 12.34.56.78
        """)
        return

    command = sys.argv[1]

    # ----------------------------------------------------------------
    # check — VPS environment inspection
    # ----------------------------------------------------------------
    if command == "check":
        args = _parse_vps_args(sys.argv)
        if not args["host"]:
            print("ERROR: --host required"); return
        try:
            d = VPSDeployer(args["host"], args["user"], args["port"], args["key"], args["password"])
            info = d.check_environment()
            print(format_env_report(info))
        except ConnectionError as e:
            print(f"Connection failed: {e}")
        except Exception as e:
            print(f"Error: {e}")

    # ----------------------------------------------------------------
    # bootstrap — initialize a fresh VPS
    # ----------------------------------------------------------------
    elif command == "bootstrap":
        args = _parse_vps_args(sys.argv)
        if not args["host"]:
            print("ERROR: --host required"); return
        web_server = "caddy" if "--caddy" in sys.argv else "nginx" if "--nginx" in sys.argv else "caddy"
        with_docker = "--docker" in sys.argv
        dry_run = "--dry-run" in sys.argv

        try:
            d = VPSDeployer(args["host"], args["user"], args["port"], args["key"], args["password"])
            print(f"Bootstrapping {args['host']} with {web_server}...")
            result = d.bootstrap(web_server=web_server, with_docker=with_docker, dry_run=dry_run)
            if "error" in result:
                print(f"ERROR: {result['error']}")
            else:
                for step in result["steps"]:
                    icon = "OK" if step["status"] == "ok" else "?" if step["status"] == "dry_run" else "FAIL"
                    print(f"  [{icon}] {step['step']}")
                print(f"\nBootstrap complete. Server ready for deployment.")
        except ConnectionError as e:
            print(f"Connection failed: {e}")
        except Exception as e:
            print(f"Error: {e}")

    # ----------------------------------------------------------------
    # deploy — one-click VPS deployment (main command)
    # ----------------------------------------------------------------
    elif command == "deploy":
        if len(sys.argv) < 4:
            print("Usage: python deployer.py deploy <project_dir> <domain> --host <ip> [options]")
            return

        project_dir = sys.argv[2]
        domain = sys.argv[3]
        args = _parse_vps_args(sys.argv)
        if not args["host"]:
            print("ERROR: --host required"); return

        modules = _parse_modules(sys.argv)
        web_server = "caddy" if "--caddy" in sys.argv else "nginx" if "--nginx" in sys.argv else "caddy"
        ssl = "--no-ssl" not in sys.argv
        dry_run = "--dry-run" in sys.argv

        try:
            d = VPSDeployer(args["host"], args["user"], args["port"], args["key"], args["password"])
            result = d.deploy(
                domain=domain,
                project_dir=project_dir,
                modules=modules,
                web_server=web_server,
                ssl=ssl,
                dry_run=dry_run,
            )

            if "error" in result:
                print(f"Deploy failed: {result['error']}")
                if "stderr" in result:
                    print(result["stderr"])
                return

            print(f"\nDeployment {'(dry-run)' if dry_run else 'complete'}!")
            for action in result["actions"]:
                icon = "OK" if action["status"] == "ok" else "?" if action["status"] in ("dry_run", "skipped") else "FAIL"
                print(f"  [{icon}] {action['step']}")
            print(f"\nSite URL: {result['url']}")
            if not dry_run and result.get("verified"):
                print("Verification: Site is LIVE")

        except ConnectionError as e:
            print(f"Connection failed: {e}")
        except Exception as e:
            print(f"Error: {e}")

    # ----------------------------------------------------------------
    # deploy-backend — FastAPI backend to VPS
    # ----------------------------------------------------------------
    elif command == "deploy-backend":
        if len(sys.argv) < 4:
            print("Usage: python deployer.py deploy-backend <backend_dir> <domain> --host <ip> [options]")
            return

        backend_dir = sys.argv[2]
        domain = sys.argv[3]
        args = _parse_vps_args(sys.argv)
        if not args["host"]:
            print("ERROR: --host required"); return

        # Parse optional flags
        dfseo_email = ""
        dfseo_password = ""
        secret = ""
        for i, arg in enumerate(sys.argv):
            if arg == "--dfseo-email" and i + 1 < len(sys.argv):
                dfseo_email = sys.argv[i + 1]
            elif arg == "--dfseo-password" and i + 1 < len(sys.argv):
                dfseo_password = sys.argv[i + 1]
            elif arg == "--secret" and i + 1 < len(sys.argv):
                secret = sys.argv[i + 1]

        if not secret:
            import secrets
            secret = secrets.token_hex(32)
            print(f"[INFO] Generated random SECRET_KEY: {secret[:16]}...")

        dry_run = "--dry-run" in sys.argv

        try:
            result = deploy_backend(
                backend_dir=backend_dir,
                domain=domain,
                host=args["host"],
                user=args["user"],
                port=args["port"],
                key_path=args["key"],
                password=args["password"],
                dataforseo_email=dfseo_email,
                dataforseo_password=dfseo_password,
                secret_key=secret,
                dry_run=dry_run,
            )

            if "error" in result:
                print(f"Deploy failed: {result['error']}")
                return

            print(f"\nBackend deployment {'(dry-run)' if dry_run else 'complete'}!")
            for action in result["actions"]:
                icon = "OK" if action["status"] == "ok" else "?" if action["status"] in ("dry_run",) else "FAIL"
                detail = f" (HTTP {action.get('http')})" if action.get("http") else ""
                err = f" — {action.get('error', '')[:100]}" if action.get("error") else ""
                print(f"  [{icon}] {action['step']}{detail}{err}")
            if not dry_run:
                print(f"\nAPI: {result['api_url']}")
                print(f"Dashboard: {result['dashboard_url']}")

        except ConnectionError as e:
            print(f"Connection failed: {e}")
        except Exception as e:
            print(f"Error: {e}")

    # ----------------------------------------------------------------
    # package — generate deployment files (offline/manual mode)
    # ----------------------------------------------------------------
    elif command == "package":
        project_dir = sys.argv[2] if len(sys.argv) > 2 else "."
        domain = sys.argv[3] if len(sys.argv) > 3 else "example.com"
        modules = _parse_modules(sys.argv)
        result = generate_deployment_package(project_dir, domain, modules)
        print(f"Created {len(result['created_files'])} deployment files:")
        for f in result["created_files"]:
            print(f"  {f}")

    # ----------------------------------------------------------------
    # vercel / cloudflare
    # ----------------------------------------------------------------
    elif command == "vercel":
        project_dir = sys.argv[2] if len(sys.argv) > 2 else "."
        result = deploy_to_vercel(project_dir)
        if "error" in result:
            print(f"Error: {result['error']}")
        else:
            print(f"Deployed to Vercel: {result['url']}")

    elif command == "cloudflare":
        project_dir = sys.argv[2] if len(sys.argv) > 2 else "."
        result = deploy_to_cloudflare(project_dir)
        if "error" in result:
            print(f"Error: {result['error']}")
        else:
            print(f"Deployed to Cloudflare Pages: {result['url']}")

    # ----------------------------------------------------------------
    # config generators (print to stdout — pipe to files)
    # ----------------------------------------------------------------
    elif command == "nginx":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        ssl = "--ssl" in sys.argv
        print(generate_nginx_config(domain, ssl=ssl, upstream_services=(
            {"directus": 8055} if "--cms" in sys.argv else None
        )))

    elif command == "caddy":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        print(generate_caddy_config(domain, upstream_services=(
            {"directus": 8055} if "--cms" in sys.argv else None
        )))

    elif command == "docker":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        modules = _parse_modules(sys.argv)
        print(generate_docker_compose(domain, modules))

    else:
        print(f"Unknown command: {command}")
        print("Run without arguments for help.")


if __name__ == "__main__":
    main()
