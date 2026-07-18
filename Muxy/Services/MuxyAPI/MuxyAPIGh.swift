import Foundation

extension MuxyAPI {
    enum Gh {
        static func user() async -> Result<GhUser, APIError> {
            switch await GhUserService.shared.user() {
            case let .success(user):
                .success(user)
            case let .failure(error):
                .failure(.underlying(error.message))
            }
        }
    }
}
