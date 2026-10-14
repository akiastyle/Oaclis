# path_analysis.py v1.0.9
"""Read-only trial using the same selector as Oaclis background analysis."""
import json
import pathlib
import sys
import environment_analysis as reader

if __name__ == '__main__':
    result = reader.select_requirements(reader.collect(pathlib.Path(sys.argv[1]), sys.argv[2]))
    print(json.dumps(result, ensure_ascii=False, indent=2))
