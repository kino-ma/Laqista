# Face Detection

ONNX model retrieved from: https://github.com/FaceONNX/FaceONNX

Inputs/outputs shape:

```
Inputs :
  -- input
     shape = [1, 3, 480, 640]
     type  = tensor(float)

Outputs :
  -- scores
     shape = [1, 17640, 2]
     type  = tensor(float)
  -- boxes
     shape = [1, 17640, 4]
     type  = tensor(float)
```
