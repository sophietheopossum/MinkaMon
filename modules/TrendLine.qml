import QtQuick
import "../services"

// Sparkline that records its own history: appends `value` on every sampler
// tick, so any instantaneous metric becomes a 60s line chart. Give it a
// stable home (model that survives ticks) or the history resets.
Sparkline {
    id: root

    property real value: 0

    Connections {
        target: Sampler

        function onTicked() {
            root.values = Sampler.pushHistory(root.values, root.value);
        }
    }
}
