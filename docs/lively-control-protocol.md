# Local wallpaper control protocol

WinGlow implements a bounded unary client for Lively 2.2.1.0's named-pipe CommandsService, independently of Livelycu. The upstream Livelycu native executable failed on this machine with `InvalidOperationException` while constructing CommandLine option types. Hash verification does not establish runtime compatibility.

Primary protocol references:

- [Lively commands.proto, v2.2.1.0](https://github.com/rocksdanister/lively/blob/v2.2.1.0/src/Lively/Lively.Grpc.Common/Proto/commands.proto): AutomationCommand's repeated UTF-8 args and the ShutDown method.
- [GrpcDotNetNamedPipes transport.proto](https://github.com/cyanfish/grpc-dotnet-namedpipes/blob/da307b551a9d56b62f5780e91a2d95a699f32430/GrpcDotNetNamedPipes/Internal/Protocol/transport.proto): delimited request, headers, payload metadata and acknowledgement trailers.
- [NamedPipeTransport](https://github.com/cyanfish/grpc-dotnet-namedpipes/blob/da307b551a9d56b62f5780e91a2d95a699f32430/GrpcDotNetNamedPipes/Internal/Protocol/NamedPipeTransport.cs), [WriteTransaction](https://github.com/cyanfish/grpc-dotnet-namedpipes/blob/da307b551a9d56b62f5780e91a2d95a699f32430/GrpcDotNetNamedPipes/Internal/Protocol/WriteTransaction.cs), [PlatformConfig](https://github.com/cyanfish/grpc-dotnet-namedpipes/blob/da307b551a9d56b62f5780e91a2d95a699f32430/GrpcDotNetNamedPipes/Internal/PlatformConfig.cs): Windows message mode, no outer 32-bit length prefix, inline small payloads.

No upstream client source is copied into WinGlow. The client accepts only bounded Empty responses with successful trailers, rejects malformed/incompatible frames, joins cancellation before releasing asynchronous I/O buffers, and retains the existing actual wallpaper-selection checks. Acknowledgement does not establish successful wallpaper rendering; real-device selection and rendering verification remains required.

