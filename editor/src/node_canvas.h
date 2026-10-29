#pragma once

#include <QGraphicsView>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QList>
#include <QPointF>
#include <QSet>
#include <QSizeF>
#include <QString>

class QGraphicsPathItem;
class QGraphicsRectItem;
class QGraphicsScene;
class QGraphicsSimpleTextItem;

// The graph as boxes and wires (DR-30, N3). The outline stays the interface a keyboard and a screen
// reader use; this view draws the same graph and adds the gestures a picture invites:
//
//   arrastar uma caixa     move (salvo em scripts/<nome>.layout.json, nunca no grafo)
//   arrastar uma saída     puxa um fio até uma entrada
//   clicar uma saída e depois uma entrada   liga as duas (sem arrastar)
//   Ctrl + clique          soma ou tira uma caixa da seleção
//   arrastar o fundo       move o canvas; botão do meio também
//   roda do mouse          rola; Ctrl + roda dá zoom de 25 % a 400 %
//   Home / F               centraliza o grafo; 0 volta a 100 %
//   Tab / setas            anda entre as caixas     Enter edita    Delete remove
//
// Every action here does the same thing the outline does, through the same operations; nothing is
// only reachable with a mouse.
class NodeCanvas : public QGraphicsView {
    Q_OBJECT

public:
    explicit NodeCanvas(QWidget* parent = nullptr);

    // Draws `graph.view`'s `canvas`: nodes with their pins and problems, and the wires.
    void setGraph(const QJsonObject& canvas);
    // What the newest frame reported: nodes that ran, and conditions that decided false.
    void setMarks(const QJsonObject& debug);
    int currentNode() const { return current_; }
    QList<int> selectedNodes() const;
    int groupOf(int node) const;
    void selectAll();
    // The sentence title of a node already drawn ("" when it is not in this graph).
    QString titleOf(int node) const;
    void selectNode(int node);
    void clearSelection();
    void setPlaying(bool playing);
    void clearMarks();

    void fitGraph();
    void resetZoom();
    void zoomIn();
    void zoomOut();

signals:
    void nodeSelected(int node);
    void nodeActivated(int node);
    void nodeRemoveRequested(int node);
    void nodesMoved(const QJsonArray& places);
    void connectRequested(const QJsonObject& from, const QJsonObject& to);
    void selectionChanged();
    void zoomChanged(int percent);
    void message(const QString& text);

protected:
    void wheelEvent(QWheelEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;

private:
    struct Pin {
        int node = 0;
        QString name;
        bool input = true;
        QPointF at;
    };

    void drawGroups(const QJsonObject& canvas);
    void drawNodes(const QJsonObject& canvas);
    void drawEdges(const QJsonObject& canvas);
    void redrawEdges();
    void paintSelection();
    void zoomBy(double factor, bool underMouse);
    void walk(int dx, int dy);
    int nodeAt(const QPointF& position) const;
    int groupAt(const QPointF& position) const;
    Pin pinAt(const QPointF& position) const;
    QPointF pinPoint(int node, const QString& pin, bool input) const;
    void describeCurrent();
    void moveNodeTo(int node, const QPointF& place);
    void updateGroupFrames();
    void refreshTempWire(const QPointF& scenePoint);
    QList<int> nodesInGroup(int group) const;

    QGraphicsScene* scene_ = nullptr;
    QHash<int, QGraphicsRectItem*> boxes_;
    QHash<int, QSizeF> boxSizes_;
    QHash<int, QJsonObject> nodes_;
    QHash<int, QPointF> places_;
    QHash<QString, QPointF> pinLocal_;
    QHash<QString, QPointF> pins_;
    QList<Pin> pinPlaces_;
    QList<QJsonObject> edges_;
    QList<QGraphicsPathItem*> wireItems_;
    QHash<int, QGraphicsRectItem*> groupFrames_;
    QHash<int, QGraphicsSimpleTextItem*> groupLabels_;
    QHash<int, QList<int>> groups_;
    QSet<int> selected_;
    QHash<int, QPointF> dragOrigins_;
    // The last graph drawn, kept so wires and pins can be laid out again after a box is dragged.
    QJsonObject canvas_;
    QList<int> ran_;
    QList<int> failed_;
    Pin pending_;
    Pin wireFrom_;
    QGraphicsPathItem* tempWire_ = nullptr;
    int current_ = 0;
    int dragNode_ = 0;
    int dragGroup_ = 0;
    QPointF dragStart_;
    bool dragging_ = false;
    bool dragMoved_ = false;
    bool wireDragging_ = false;
    bool wireMoved_ = false;
    bool panning_ = false;
    QPoint lastPan_;
    double zoom_ = 1.0;
    bool hasGraph_ = false;
};
