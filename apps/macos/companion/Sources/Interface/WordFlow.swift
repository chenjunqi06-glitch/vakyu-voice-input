// 词元按可用宽度换行，中文拼音与字同行；长句放在可滚动区域内。
import SwiftUI

struct WordFlow: Layout {
    var spacing: CGFloat = 5

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = positions(width: proposal.width ?? 640, subviews: subviews)
        return CGSize(width: proposal.width ?? 640, height: rows.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = positions(width: bounds.width, subviews: subviews)
        for index in subviews.indices {
            subviews[index].place(at: CGPoint(x: bounds.minX + result.points[index].x,
                                             y: bounds.minY + result.points[index].y),
                                  proposal: .unspecified)
        }
    }

    private func positions(width: CGFloat, subviews: Subviews) -> (points: [CGPoint], height: CGFloat) {
        var points: [CGPoint] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width { x = 0; y += rowHeight + spacing; rowHeight = 0 }
            points.append(CGPoint(x: x, y: y))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
        return (points, y + rowHeight)
    }
}
