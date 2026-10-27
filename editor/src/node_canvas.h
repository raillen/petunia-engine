#pragma once

#include <QGraphicsView>
#include <QHash>
#include <QJsonObject>
#include <QList>
#include <QPointF>
#include <QString>

class QGraphicsPathItem;
class QGraphicsRectItem;
class QGraphicsScene;

// The graph as boxes and wires (DR-30, N3). The outline stays the interface a keyboard and a screen
// reader use; this view draws the same graph and adds the gestures a picture invites:
//
//   arrastar uma caixa     move (salvo em scripts/<nome>.layout.json, nunca no grafo)
//   clicar uma caixa       seleciona (o contorno acompanha)
//   Tab / setas            anda entre as caixas     Enter edita    Delete remove
//   clicar uma saída e depois uma entrada   liga as duas
//   Ctrl + roda, + / -     zoom de 25 % a 400 %
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
    // The sentence title of a node already drawn ("" when it is not in this graph).
    QString titleOf(int node) const;
    void selectNode(int node);
    void setPlaying(bool playing);
    void clearMarks();

signals:
    void nodeSelected(int node);
    void nodeActivated(int node);
    void nodeRemoveRequested(int node);
    void nodeMoved(int node, double x, double y);
    void connectRequested(const QJsonObject& from, const QJsonObject& to);
    void message(const QString& text);

protected:
    void wheelEvent(QWheelEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;

private:
    struct Pin {
        int node = 0;
        QString name;
        bool input = true;
        QPointF at;
    };

    void drawNodes(const QJsonObject& canvas);
    void drawEdges(const QJsonObject& canvas);
    void paintSelection();
    void zoomBy(double factor);
    void walk(int dx, int dy);
    int nodeAt(const QPointF& position) const;
    Pin pinAt(const QPointF& position) const;
    QPointF pinPoint(int node, const QString& pin, bool input) const;
    void describeCurrent();

    QGraphicsScene* scene_ = nullptr;
    QHash<int, QGraphicsRectItem*> boxes_;
    QHash<int, QJsonObject> nodes_;
    QHash<int, QPointF> places_;
    QHash<QString, QPointF> pins_;
    QList<Pin> pinPlaces_;
    // The last graph drawn, kept so wires and pins can be laid out again after a box is dragged.
    QJsonObject canvas_;
    QList<int> ran_;
    QList<int> failed_;
    Pin pending_;
    int current_ = 0;
    double zoom_ = 1.0;
};
