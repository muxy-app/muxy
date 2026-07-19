import Darwin
import Foundation

guard let command = AgentHookCommand.parse(Array(CommandLine.arguments.dropFirst())) else {
    exit(EXIT_SUCCESS)
}

let input = FileHandle.standardInput.readDataToEndOfFile()
let result = AgentHookRuntime().run(command: command, input: input)

switch result {
case .success:
    exit(EXIT_SUCCESS)
case let .failure(message):
    FileHandle.standardError.write(Data(message.utf8))
    exit(EXIT_FAILURE)
}
