import sys
import os

sys.path.insert(0, "/www/seoplant-backend")
os.chdir("/www/seoplant-backend")

try:
    from backend.main import app
    print("IMPORT_OK")
except Exception as e:
    print(f"IMPORT_ERROR: {e}")
