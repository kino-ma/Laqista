import sys

import onnx

softmax = onnx.helper.make_node("Softmax", inputs=["X"], outputs=["Y"])

X = onnx.helper.make_tensor_value_info("X", onnx.TensorProto.FLOAT, shape=[1, 1000])
Y = onnx.helper.make_tensor_value_info("Y", onnx.TensorProto.FLOAT, shape=[1, 1000])

graph = onnx.helper.make_graph(
    [softmax],
    "softmax",
    inputs=[X],
    outputs=[Y],
)

model = onnx.helper.make_model(graph)

onnx.checker.check_model(model)

onnx.save(model, sys.argv[1])
print(f"Saved Softmax graph to {sys.argv[1]}")
