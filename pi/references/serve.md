# Registry Container Mode (HTTP Server)

Serve a Regedited document over HTTP as a REST API. Enables remote registry access, containerized configuration, and CI-friendly queries.

## Start the Server

```bash
# Basic (read-only, port 5000)
regedited serve --file config.regd --port 5000

# Writable mode (allow modifications via API)
regedited serve --file config.regd --port 5000 --read-only false
```

## Endpoints

### Status & Discovery

| Method | Path | Returns |
|--------|------|---------|
| GET | `/` | Server status, section count, read-only flag |
| GET | `/health` | `{status: "healthy", sections: N}` |
| GET | `/sections` | All sections with metadata |

### Section Access

| Method | Path | Returns |
|--------|------|---------|
| GET | `/section/{name}` | Section metadata (header line, content range, db line) |
| GET | `/section/{name}/db` | Database table (9 values + 3 strings) |
| GET | `/section/{name}/ascii` | Hex-word store (6 typed zone pairs) |
| GET | `/section/{name}/zone/{i}` | Zone content as JSON |

### Search & Query

| Method | Path | Returns |
|--------|------|---------|
| GET | `/grep?pattern={p}&section={s}` | Lines matching pattern |
| POST | `/query` | Boolean query results |

### Utilities

| Method | Path | Returns |
|--------|------|---------|
| GET | `/types` | Registry types list |
| GET | `/wal` | WAL status |

## curl Examples

```bash
# List all sections
curl http://localhost:5000/sections

# Get Config section metadata
curl http://localhost:5000/section/Config

# Get Config database table
curl http://localhost:5000/section/Config/db

# Get zone 0 content
curl http://localhost:5000/section/Config/zone/0

# Search for "enabled" in all sections
curl "http://localhost:5000/grep?pattern=enabled"

# Search in specific section
curl "http://localhost:5000/grep?pattern=enabled&section=Config"

# Health check
curl http://localhost:5000/health
```

## Python Client

```python
import requests

BASE = "http://localhost:5000"

# List sections
r = requests.get(f"{BASE}/sections")
sections = r.json()

# Get section metadata
r = requests.get(f"{BASE}/section/Config")
config = r.json()

# Get zone content
r = requests.get(f"{BASE}/section/Config/zone/0")
zone = r.json()["content"]

# Search
r = requests.get(f"{BASE}/grep", params={"pattern": "enabled"})
matches = r.json()["matches"]
```

## Container Usage

```bash
# Dockerfile example
FROM alpine:latest
COPY config.regd /data/config.regd
COPY regedited /usr/local/bin/regedited
EXPOSE 5000
CMD ["regedited", "serve", "--file", "/data/config.regd", "--port", "5000"]
```

```bash
# docker run
docker build -t my-registry .
docker run -p 5000:5000 my-registry
```

## CI/CD Integration

```yaml
# .github/workflows/config-check.yml
- name: Verify registry config
  run: |
    curl -sf http://localhost:5000/health || exit 1
    curl -sf "http://localhost:5000/grep?pattern=production" || exit 1
```
