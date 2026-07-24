import Darwin
import Foundation
import MuxyHookKit

guard let command = AgentHookCommand.parse(Array(CommandLine.arguments.dropFirst())) else {
    exit(EXIT_SUCCESS)
}

let budget = AgentHookExecutionBudget()
let input = AgentHookStandardInput.read(
    descriptor: FileHandle.standardInput.fileDescriptor,
    limit: AgentHookStandardInput.maximumPayloadBytes,
    budget: budget
)
let result = AgentHookRuntime().run(command: command, input: input, budget: budget)

switch result {
case .success:
    exit(EXIT_SUCCESS)
case let .failure(message):
    FileHandle.standardError.write(Data(message.utf8))
    exit(EXIT_FAILURE)
}
