import sys

import onnx

import onnxruntime

model = onnx.load(sys.argv[1])
onnx.checker.check_model(model)

print(f"Model Version:\t{model.model_version}")
print(f"IR Version:\t{model.ir_version}")
# print(f"Serialized:\n{model.SerializeToString()}")

print("Graph:")
print(onnx.helper.printable_graph(model.graph))

session = onnxruntime.InferenceSession(sys.argv[1])

print("Inputs :")

for input in session.get_inputs():
    print("  --", input.name)
    print("     shape =", input.shape)
    print("     type  =", input.type)

print()
print("Outputs :")

for output in session.get_outputs():
    print("  --", output.name)
    print("     shape =", output.shape)
    print("     type  =", output.type)
