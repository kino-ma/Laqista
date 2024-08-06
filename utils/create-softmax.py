import sys

import onnx

softmax = onnx.helper.make_node(
    "Softmax", inputs=["squeezenet0_flatten0_reshape0"], outputs=["probabilities"]
)

squeezenet0_flatten0_reshape0 = onnx.helper.make_tensor_value_info(
    "squeezenet0_flatten0_reshape0", onnx.TensorProto.FLOAT, shape=[1, 1000]
)
probabilities = onnx.helper.make_tensor_value_info(
    "probabilities", onnx.TensorProto.FLOAT, shape=[1, 1000]
)

graph = onnx.helper.make_graph(
    [softmax],
    "softmax",
    inputs=[squeezenet0_flatten0_reshape0],
    outputs=[probabilities],
)

model = onnx.helper.make_model(graph)

onnx.checker.check_model(model)

onnx.save(model, sys.argv[1])
print(f"Saved Softmax graph to {sys.argv[1]}")
