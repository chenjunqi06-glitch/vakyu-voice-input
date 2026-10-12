// 本地离线词条，不保存用户点击记录或发送网络查询。
import Foundation

struct WordCard: Decodable, Equatable {
    let word: String

    let meaning: String?

    let pronunciation: String?

    let hint: String?
}
