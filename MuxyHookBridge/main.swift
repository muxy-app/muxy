import Darwin
import Foundation

guard let command = AgentHookCommand.parse(Array(CommandLine.arguments.dropFirst())) else {
    exit(EXIT_SUCCESS)
}

let input = FileHandle.standardInput.readDataToEndOfFile()
AgentHookRuntime().run(command: command, input: input)
exit(EXIT_SUCCESS)
