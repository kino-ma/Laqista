import json
import subprocess
import sys

DATA_FILENAME = "tmp.dat"
PLOT_FILENAME = "tmp.plt"

json_filename = sys.argv[1]
output_filename = sys.argv[2]

data = None
with open(json_filename) as f:
    data = json.loads(f.read())

script = f"""set terminal svg
set output "{output_filename}"
plot "{DATA_FILENAME}"
"""

lines = []
for i, time in enumerate(data["times"], start=1):
    line = f"{i}\t{time}"
    lines.append(line)

iterations_data = "\n".join(lines)

with open(DATA_FILENAME, "w") as f:
    f.write(iterations_data)

with open(PLOT_FILENAME, "w") as f:
    f.write(script)

subprocess.run(["gnuplot", PLOT_FILENAME])
