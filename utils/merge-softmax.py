import sys

import onnx

model_path = sys.argv[1]
softmax_path = sys.argv[2]
output_path = sys.argv[3]

model = onnx.load(model_path)
softmax = onnx.load(softmax_path)

onnx.checker.check_model(model)
onnx.checker.check_model(softmax)

merged = onnx.compose.merge_models(
    model,
    softmax,
    io_map=[("972", "X")],
)

onnx.save(merged, output_path)

print(f"Saved marged model to {output_path}")
