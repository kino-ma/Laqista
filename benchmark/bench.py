#!/usr/bin/env python3

import grpc

import gen
import gen.mless_pb2 as pb2
import gen.mless_pb2_grpc as pb2_grpc

# from grpc.mless_pb2_grpc import SchedulerStub


def main():
    channel = grpc.insecure_channel("http://127.0.0.1:50051")
    stub = pb2_grpc.SchedulerStub(channel)

    call(stub)


def call(stub):
    request = pb2.LookupRequest(deployment_id="")
    resp = stub.Lookup(request)

    print(resp)


if __name__ == "__main__":
    main()
