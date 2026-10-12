//! Miracle addon 工厂；插件注册 ID 继续保留 qingjian 以兼容现有 Fcitx 配置。
#include "miracle.h"
namespace fcitx {
class MiracleFactory final : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override { return new MiracleEngine(manager); }
};
}
FCITX_ADDON_FACTORY_V2(qingjian, fcitx::MiracleFactory)
