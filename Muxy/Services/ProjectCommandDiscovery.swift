import Foundation

protocol ProjectCommandProviding {
    func commands(projectPath: String) -> [ProjectCommand]
}

struct ProjectCommandDiscovery {
    var providers: [any ProjectCommandProviding]

    init(providers: [any ProjectCommandProviding] = [NPMProjectCommandProvider(), ComposerProjectCommandProvider()]) {
        self.providers = providers
    }

    func commands(projectPath: String) -> [ProjectCommand] {
        providers.flatMap { $0.commands(projectPath: projectPath) }
            .sorted { lhs, rhs in
                if lhs.source != rhs.source { return lhs.source.title < rhs.source.title }
                return lhs.name.localizedStandardCompare(rhs.name) == .orderedAscending
            }
    }
}

struct NPMProjectCommandProvider: ProjectCommandProviding {
    func commands(projectPath: String) -> [ProjectCommand] {
        let url = URL(fileURLWithPath: projectPath).appendingPathComponent("package.json")
        guard let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let scripts = object["scripts"] as? [String: String]
        else { return [] }

        return scripts.keys.sorted().map { script in
            ProjectCommand(
                id: "npm:\(script)",
                name: script,
                command: "npm run \(ShellEscaper.escape(script))",
                source: .npm
            )
        }
    }
}

struct ComposerProjectCommandProvider: ProjectCommandProviding {
    func commands(projectPath: String) -> [ProjectCommand] {
        let url = URL(fileURLWithPath: projectPath).appendingPathComponent("composer.json")
        guard let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let scripts = object["scripts"] as? [String: Any]
        else { return [] }

        return scripts.keys.sorted().map { script in
            ProjectCommand(
                id: "composer:\(script)",
                name: script,
                command: "composer run-script \(ShellEscaper.escape(script))",
                source: .composer
            )
        }
    }
}
