#!/bin/bash
# Setup nginx reverse proxy for SEOplant backend

cat > /etc/nginx/sites-available/api.seoplant.io <<'NGINX'
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
NGINX

ln -sf /etc/nginx/sites-available/api.seoplant.io /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

nginx -t && systemctl restart nginx
echo "Nginx reloaded"

# Test
curl -s -H "Host: api.seoplant.io" http://127.0.0.1/api/auth/me
echo ""
echo "---"
systemctl status seoplant-api --no-pager | grep Active
