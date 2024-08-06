import sys

import onnx
from onnx import version_converter

model_path = sys.argv[1]
onnx_ir_version = int(sys.argv[2])
output_path = sys.argv[3]

model = onnx.load(model_path)

converted = version_converter.convert_version(model, onnx_ir_version)
onnx.checker.check_model(converted)

onnx.save(converted, output_path)
print(f"Saved Up/Downgraded graph to {output_path}")
