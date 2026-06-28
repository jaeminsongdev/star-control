#!/usr/bin/env python3
from pathlib import Path
import sys
import yaml

root = Path(__file__).resolve().parents[2]
path = root / 'builtin-tools/star-sentinel/tool.yaml'
with path.open('r', encoding='utf-8') as file:
    data = yaml.safe_load(file)
required = ['id', 'name', 'kind', 'package', 'entrypoint', 'commands', 'profiles', 'outputs']
missing = [key for key in required if key not in data]
if missing:
    print('manifest contract check failed: ' + ', '.join(missing), file=sys.stderr)
    raise SystemExit(1)
print('manifest contract check passed')
