#!/bin/bash
set -e

# Move to valid Python package name
mv /www/seoplant-backend /www/backend

# Update systemd service
cat > /etc/systemd/system/seoplant-api.service <<'SVCEND'
[Unit]
Description=SEOplant API Backend
After=network.target

[Service]
Type=simple
User=nobody
WorkingDirectory=/www
EnvironmentFile=/www/backend/.env
ExecStart=/www/backend/venv/bin/python3 -m backend.main
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SVCEND

systemctl daemon-reload
systemctl restart seoplant-api
sleep 3
systemctl status seoplant-api --no-pager -l

echo "---"
echo "Recent logs:"
journalctl -u seoplant-api --no-pager -n 10
