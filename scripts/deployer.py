"""
Deployer — Phase 5 of website-generator skill.

Generates deployment configurations: Nginx, Docker Compose,
deployment scripts, and analytics setup.

Dependencies: none (standard library only)
"""

import json
import sys
from datetime import datetime
from pathlib import Path


# ---------------------------------------------------------------------------
# 1. Nginx Configuration
# ---------------------------------------------------------------------------

def generate_nginx_config(
    domain: str,
    root_path: str = "/www/wwwroot/{domain}/dist",
    ssl: bool = False,
    upstream_services: dict = None,
) -> str:
    """Generate Nginx configuration for Baota panel or manual setup.

    Args:
        domain: The website domain
        root_path: Path to static files (use {domain} as placeholder)
        ssl: Whether to include SSL configuration
        upstream_services: Dict of service_name -> port for reverse proxy
    """
    upstream_services = upstream_services or {}
    root = root_path.replace("{domain}", domain)

    # Upstream blocks
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

    listen_block = "listen 80;" if not ssl else """listen 80;
    listen 443 ssl http2;
    ssl_certificate /www/server/panel/vhost/cert/{domain}/fullchain.pem;
    ssl_certificate_key /www/server/panel/vhost/cert/{domain}/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # HTTP -> HTTPS redirect
    if ($server_port = 80) {{
        return 301 https://$host$request_uri;
    }}""".replace("{domain}", domain)

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
    access_log /www/wwwlogs/{domain}.log;
    error_log /www/wwwlogs/{domain}.error.log;
}}
"""


# ---------------------------------------------------------------------------
# 2. Docker Compose
# ---------------------------------------------------------------------------

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

    # Build YAML manually (avoid PyYAML dependency)
    yaml_lines = ["version: '3.8'", "", "services:"]

    for name, config in services.items():
        yaml_lines.append(f"  {name}:")
        for key, value in config.items():
            if key == "environment" and isinstance(value, dict):
                yaml_lines.append(f"    environment:")
                for env_key, env_val in value.items():
                    yaml_lines.append(f"      {env_key}: \"{env_val}\"")
            elif key == "volumes" and isinstance(value, list):
                yaml_lines.append(f"    volumes:")
                for v in value:
                    yaml_lines.append(f"      - {v}")
            elif key == "ports" and isinstance(value, list):
                yaml_lines.append(f"    ports:")
                for p in value:
                    yaml_lines.append(f"      - \"{p}\"")
            elif key == "depends_on" and isinstance(value, list):
                yaml_lines.append(f"    depends_on:")
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


# ---------------------------------------------------------------------------
# 3. Deployment Script
# ---------------------------------------------------------------------------

def generate_deploy_script(
    domain: str,
    server_user: str = "root",
    server_host: str = "your-server-ip",
    deploy_path: str = "/www/wwwroot/{domain}",
    has_docker: bool = False,
) -> str:
    """Generate deploy.sh script."""
    path = deploy_path.replace("{domain}", domain)

    script = f"""#!/bin/bash
# Deployment script for {domain}
# Generated: {datetime.now().strftime('%Y-%m-%d')}

set -e

echo "🏗️  Building site..."
npm run build

echo "📦 Deploying to {server_host}..."
rsync -avz --delete \\
  --exclude '.git' \\
  --exclude 'node_modules' \\
  dist/ {server_user}@{server_host}:{path}/dist/

echo "🔄 Reloading Nginx..."
ssh {server_user}@{server_host} "nginx -t && nginx -s reload"
"""

    if has_docker:
        script += f"""
echo "🐳 Updating Docker services..."
scp docker-compose.yml {server_user}@{server_host}:{path}/
ssh {server_user}@{server_host} "cd {path} && docker compose pull && docker compose up -d"
"""

    script += """
echo "✅ Deployment complete!"
echo "🌐 Visit: https://{domain}"
""".replace("{domain}", domain)

    return script


# ---------------------------------------------------------------------------
# 4. Analytics Setup
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# 5. Full Deployment Package
# ---------------------------------------------------------------------------

def generate_deployment_package(
    project_dir: str,
    domain: str,
    modules: dict = None,
    server_user: str = "root",
    server_host: str = "your-server-ip",
    ssl: bool = True,
) -> dict:
    """Generate all deployment files in the deploy/ directory.

    Returns:
        Dict with list of created files
    """
    modules = modules or {}
    base = Path(project_dir) / "deploy"
    base.mkdir(parents=True, exist_ok=True)
    created = []

    # Upstream services for Nginx
    upstreams = {}
    if modules.get("cms"):
        upstreams["directus"] = 8055
    if modules.get("ecommerce"):
        upstreams["medusa"] = 9000
    if modules.get("analytics"):
        tool = modules.get("analytics_tool", "plausible")
        if tool == "plausible":
            upstreams["plausible"] = 8000
        elif tool == "tianji":
            upstreams["tianji"] = 12345

    # 1. Nginx config
    nginx = generate_nginx_config(domain, ssl=ssl, upstream_services=upstreams)
    (base / f"{domain}.conf").write_text(nginx, encoding="utf-8")
    created.append(f"deploy/{domain}.conf")

    # 2. Docker Compose (if services needed)
    if modules.get("cms") or modules.get("ecommerce") or modules.get("analytics"):
        compose = generate_docker_compose(domain, modules)
        (base / "docker-compose.yml").write_text(compose, encoding="utf-8")
        created.append("deploy/docker-compose.yml")

    # 3. Deploy script
    has_docker = bool(modules.get("cms") or modules.get("ecommerce") or modules.get("analytics"))
    deploy_sh = generate_deploy_script(domain, server_user, server_host, has_docker=has_docker)
    deploy_path = base / "deploy.sh"
    deploy_path.write_text(deploy_sh, encoding="utf-8")
    created.append("deploy/deploy.sh")

    # 4. Analytics snippet
    if modules.get("analytics"):
        tool = modules.get("analytics_tool", "plausible")
        snippet = generate_analytics_snippet(domain, tool)
        (base / "analytics-snippet.html").write_text(snippet, encoding="utf-8")
        created.append("deploy/analytics-snippet.html")

    # 5. README
    readme = f"""# Deployment Guide for {domain}

## Files
- `{domain}.conf` — Nginx configuration (copy to Baota panel or /etc/nginx/sites-available/)
- `docker-compose.yml` — Docker services (Directus, analytics, etc.)
- `deploy.sh` — Automated deployment script
- `analytics-snippet.html` — Analytics tracking code

## Quick Deploy

```bash
# Make deploy script executable
chmod +x deploy/deploy.sh

# Run deployment
./deploy/deploy.sh
```

## Manual Steps

1. Copy `{domain}.conf` to your Nginx configuration
2. Run `nginx -t` to validate
3. Run `nginx -s reload` to apply
4. If using Docker services: `docker compose up -d`
5. Add analytics snippet to your BaseLayout.astro `<head>`

Generated: {datetime.now().strftime('%Y-%m-%d %H:%M')}
"""
    (base / "README.md").write_text(readme, encoding="utf-8")
    created.append("deploy/README.md")

    return {"created_files": created}


# ---------------------------------------------------------------------------
# CLI Interface
# ---------------------------------------------------------------------------

def main():
    # Fix Windows GBK encoding crash when printing emoji / Unicode
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2:
        print("""
Usage:
  python deployer.py nginx example.com [--ssl]
  python deployer.py docker example.com [--cms] [--analytics] [--ecommerce]
  python deployer.py script example.com [--user root] [--host 1.2.3.4]
  python deployer.py package ./project example.com [--cms] [--analytics]
        """)
        return

    command = sys.argv[1]

    if command == "nginx":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        ssl = "--ssl" in sys.argv
        print(generate_nginx_config(domain, ssl=ssl))

    elif command == "docker":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        modules = {
            "cms": "--cms" in sys.argv,
            "analytics": "--analytics" in sys.argv,
            "ecommerce": "--ecommerce" in sys.argv,
        }
        print(generate_docker_compose(domain, modules))

    elif command == "script":
        domain = sys.argv[2] if len(sys.argv) > 2 else "example.com"
        user = "root"
        host = "your-server-ip"
        for i, arg in enumerate(sys.argv):
            if arg == "--user" and i + 1 < len(sys.argv):
                user = sys.argv[i + 1]
            if arg == "--host" and i + 1 < len(sys.argv):
                host = sys.argv[i + 1]
        print(generate_deploy_script(domain, user, host))

    elif command == "package":
        project_dir = sys.argv[2] if len(sys.argv) > 2 else "."
        domain = sys.argv[3] if len(sys.argv) > 3 else "example.com"
        modules = {
            "cms": "--cms" in sys.argv,
            "analytics": "--analytics" in sys.argv,
            "ecommerce": "--ecommerce" in sys.argv,
        }
        result = generate_deployment_package(project_dir, domain, modules)
        print(f"✅ Created {len(result['created_files'])} deployment files:")
        for f in result["created_files"]:
            print(f"  📄 {f}")

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()
