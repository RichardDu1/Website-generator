#!/bin/bash
# Fix nginx dpkg-new files
cd /etc/nginx
for f in *.dpkg-new; do
    newname=$(echo "$f" | sed 's/\.dpkg-new//')
    mv "$f" "$newname"
done

# Create API reverse proxy config
cat > /etc/nginx/sites-available/api.seoplant.io <<'NGX'
server {
    listen 80;
    server_name api.seoplant.io;

    location / {
        proxy_pass http://127.0.0.1:8800;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
NGX

ln -sf /etc/nginx/sites-available/api.seoplant.io /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

echo "=== nginx -t ==="
nginx -t 2>&1
echo "=== Reload ==="
nginx -s reload 2>&1
echo "=== Local test ==="
curl -s http://127.0.0.1:8800/api/auth/me
echo ""
echo "=== Service status ==="
systemctl status seoplant-api --no-pager | grep Active
