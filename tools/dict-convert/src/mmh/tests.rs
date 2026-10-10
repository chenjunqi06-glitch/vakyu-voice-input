//! mmh-reference 的首笔几何分类测试。

use crate::mmh::first_class;

/// 坐标就是 1024 见方里的原始值，分类器自己归一。
#[test]
fn classifies_straight_directions() {
    // 横：向右微上
    assert_eq!(first_class(&[[100.0, 500.0], [700.0, 510.0]]), 'h');
    // 提：向右上，按「提归横」记 h
    assert_eq!(first_class(&[[300.0, 300.0], [700.0, 500.0]]), 'h');
    // 竖：近竖直向下（y 向上，向下是负）
    assert_eq!(first_class(&[[500.0, 700.0], [510.0, 200.0]]), 's');
    // 撇：向左下
    assert_eq!(first_class(&[[600.0, 800.0], [300.0, 400.0]]), 'p');
    // 平撇：浅浅地向左下，仍然是撇
    assert_eq!(first_class(&[[600.0, 650.0], [200.0, 600.0]]), 'p');
    // 捺：向右下
    assert_eq!(first_class(&[[400.0, 600.0], [700.0, 300.0]]), 'n');
}

#[test]
fn classifies_turns_short_and_degenerate() {
    // 横折：先右后下，一处尖角
    assert_eq!(
        first_class(&[[200.0, 700.0], [600.0, 710.0], [610.0, 300.0]]),
        'z'
    );
    // 竖提：先下后右上，一处尖角
    assert_eq!(
        first_class(&[[500.0, 700.0], [520.0, 300.0], [650.0, 280.0]]),
        'z'
    );
    // 撇是渐弯的：一段只偏一点，不成尖角
    assert_eq!(
        first_class(&[
            [700.0, 800.0],
            [600.0, 700.0],
            [500.0, 620.0],
            [420.0, 560.0]
        ]),
        'p'
    );
    // 点：太短分不出走向，记 n
    assert_eq!(first_class(&[[500.0, 600.0], [515.0, 589.0]]), 'n');
    // 单点折线没有方向
    assert_eq!(first_class(&[[500.0, 500.0]]), '?');
}
